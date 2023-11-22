use core::slice::SlicePattern;
use std::collections::HashMap;
use std::iter::{Map, Take};
use std::marker::PhantomData;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::slice::Iter;

use log::info;
use tokio::join;
use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends, Buffer, BindGroup, ComputePipeline, BufferSlice, MapMode, Device, Queue, SubmissionIndex, BufferView, Adapter};

use crate::buffers::BufferManager;
use crate::chess::{GpuBoard, Side, EvalScore};
use crate::shaders::{Shader, self, BuffOffsets};
use crate::misc::SliceExtension;

const WORKGROUP_SIZE: u64 = 64;

pub struct GpuGlobalData {
    pub device: Rc<Device>,
    pub queue: Queue,
    pub global_data: Buffer,
    pub just_zero: Buffer,
    pub out_index: Buffer,
    pub out_index_staging: Buffer,
    pub expand_shader: Shader,
    pub eval_contract_shader: Shader,
    pub contract_shader: Shader,
    pub fill_max_shader: Shader,
}

impl GpuGlobalData {
    pub fn set_all_global_data(&self, input_size: u32, to_move: Side, move_num: u32, offsets: BuffOffsets) {
        assert!(move_num == 0);
        let mut data = [0; 28];
        data[0..4].copy_from_slice(&(input_size as u32).to_le_bytes());
        data[4..8].copy_from_slice(bytemuck::bytes_of(&to_move.gpu_representation()));
        data[8..12].copy_from_slice(bytemuck::bytes_of(&move_num));
        data[12..28].copy_from_slice(bytemuck::bytes_of(&offsets));
        self.queue.write_buffer(&self.global_data, 0, &data);
    }
}

pub struct SelfClosingBufferView<'a, const N: usize, F> {
    buf_view: Option<BufferView<'a>>,
    amount: usize,
    buf: &'a Buffer,
    func: F,
}

impl <const N: usize, F, T> SelfClosingBufferView<'_, N, F> where F: Fn(&[u8; N]) -> T {
    pub fn get_size(&self) -> usize {
        return self.amount;
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        // Safety: buf_view is always Some at this point
        let chunks = self.buf_view.as_ref().unwrap().as_chunks::<N>().0;
        let iter = chunks.iter().take(self.amount).map(|b| {(self.func)(b)});
        return iter;
    }
}

impl <const N: usize, F> Drop for SelfClosingBufferView<'_, N, F> {
    fn drop(&mut self) {
        let view = self.buf_view.take();
        drop(view);
        self.buf.unmap();
    }
}

pub async fn init_gpu_evaluator(adapter: &Adapter) -> GpuGlobalData {
    info!("Using gpu adapter: {:?}", adapter.get_info());

    let (device, queue) = adapter.request_device(&DeviceDescriptor::default(), None).await.expect("Failed to open GPU");

    let global_data = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Global Data Uniform"),
            size: 7 * size_of::<u32>() as u64, 
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST, 
            mapped_at_creation: false
        }
    );

    let just_zero = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Zero"),
            size: 1 * size_of::<u32>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }
    );

    let out_index = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Output Index Atomic"),
            size: 1 * size_of::<u32>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST, 
            mapped_at_creation: false
        }
    );

    let out_index_staging = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Output Index Staging"),
            size: 1 * size_of::<u32>() as u64, 
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
            mapped_at_creation: false
        }
    );

    let expand_shader = shaders::expand(&device);
    let eval_contract_shader = shaders::eval_contract(&device);
    let contract_shader = shaders::contract(&device);
    let fill_max_shader = shaders::fill_max(&device);

    let device_rc = Rc::new(device);

    return GpuGlobalData {
        device: device_rc,
        queue,
        global_data,
        just_zero,
        out_index,
        out_index_staging,
        expand_shader,
        eval_contract_shader,
        contract_shader,
        fill_max_shader
    };
}

pub async fn init_adapter() -> Adapter {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends:wgpu::Backends::all(),
        dx12_shader_compiler: Default::default(),
        flags: Default::default(),
        gles_minor_version: Default::default(),
    });
    let adapter = instance.request_adapter(&&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: None,
    }).await.expect("WebGPU no does work :(");
    return adapter;
}

pub struct GpuAllocations {
    pub boards: BufferManager<GpuBoard>,
    pub evals: BufferManager<EvalScore>,
    /// The amount of boards per buffer (for buffers that contain boards)
    boards_per_buf: u64,
    /// The size of buffers (for buffers that contain boards) (in bytes)
    buffer_size: u64,
}

impl<'dev> GpuAllocations {
    pub fn init(device: Rc<Device>) -> Self {
        // Buffer size calculations
        let max_buffer_size = u64::min(device.limits().max_buffer_size, device.limits().max_storage_buffer_binding_size as u64);
        let max_boards_per_buf = max_buffer_size / size_of::<GpuBoard>() as u64;
        info!("Max buffer size is {max_buffer_size}, which fits {max_boards_per_buf} boards");
        let max_dispatch = device.limits().max_compute_workgroups_per_dimension as u64;
        let max_boards_dispatch = max_dispatch * WORKGROUP_SIZE;
        info!("Max dispatch is {max_dispatch}, which fits {max_boards_dispatch} boards");
        let mut boards_per_buf = u64::min(max_boards_per_buf, max_boards_dispatch);
        if cfg!(test) {
            info!("Detected test-mode, downsizing buffers");
            boards_per_buf = 512;
        }
        let buffer_size = boards_per_buf * size_of::<GpuBoard>() as u64;
        info!("We're allocating buffers of size {buffer_size}, which fits {boards_per_buf} boards");

        let boards = BufferManager::create(device.clone(), boards_per_buf, "Board storage");
        let evals = BufferManager::create(device.clone(), boards_per_buf, "Eval storage");

        return Self {boards, evals, boards_per_buf,buffer_size};
    }

    // pub fn create_combo(&self, parent: u8, child: u8, engine: &GpuChessEvaluator) -> BufferCombo {
    //     let expansion_bind = engine.device.create_bind_group(
    //         &BindGroupDescriptor {
    //             label: None,
    //             layout: &engine.expand_shader.0,
    //             entries: &[
    //                 BindGroupEntry {
    //                     binding: 0,
    //                     resource: wgpu::BindingResource::Buffer(self.board_buffers[parent as usize].as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 1,
    //                     resource: wgpu::BindingResource::Buffer(self.board_buffers[child as usize].as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 2,
    //                     resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 3,
    //                     resource: wgpu::BindingResource::Buffer(engine.out_index.as_entire_buffer_binding())
    //                 },
    //             ]
    //         }
    //     );

    //     let eval_contract_bind = engine.device.create_bind_group(
    //         &BindGroupDescriptor {
    //             label: None,
    //             layout: &engine.eval_contract_shader.0,
    //             entries: &[
    //                 BindGroupEntry {
    //                     binding: 0,
    //                     resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 1,
    //                     resource: wgpu::BindingResource::Buffer(self.board_buffers[child as usize].as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 2,
    //                     resource: wgpu::BindingResource::Buffer(self.eval_buffers[parent as usize].as_entire_buffer_binding())
    //                 },
    //             ]
    //         }
    //     );

    //     let contract_bind = engine.device.create_bind_group(
    //         &BindGroupDescriptor {
    //             label: None,
    //             layout: &engine.contract_shader.0,
    //             entries: &[
    //                 BindGroupEntry {
    //                     binding: 0,
    //                     resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 1,
    //                     resource: wgpu::BindingResource::Buffer(self.board_buffers[child as usize].as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 2,
    //                     resource: wgpu::BindingResource::Buffer(self.eval_buffers[child as usize].as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 3,
    //                     resource: wgpu::BindingResource::Buffer(self.eval_buffers[parent as usize].as_entire_buffer_binding())
    //                 },
    //             ]
    //         }
    //     );

    //     let fill_max_bind = engine.device.create_bind_group(
    //         &BindGroupDescriptor {
    //             label: None,
    //             layout: &engine.fill_max_shader.0,
    //             entries: &[
    //                 BindGroupEntry {
    //                     binding: 0,
    //                     resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
    //                 },
    //                 BindGroupEntry {
    //                     binding: 1,
    //                     resource: wgpu::BindingResource::Buffer(self.eval_buffers[parent as usize].as_entire_buffer_binding())
    //                 },
    //             ]
    //         }
    //     );

    //     return BufferCombo {
    //         expansion_bind,
    //         eval_contract_bind,
    //         contract_bind,
    //         fill_max_bind,
    //         parent,
    //         child,
    //     };
    // }
}

pub struct BufferCombo {
    parent: u8,
    child: u8,
    expansion_bind: BindGroup,
    eval_contract_bind: BindGroup,
    contract_bind: BindGroup,
    fill_max_bind: BindGroup,
}

fn ceil_div(a: u32, b: u64) -> u32 {
    return (a as f64 / b as f64).ceil() as u32;
}