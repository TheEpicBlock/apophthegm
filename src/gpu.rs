use core::slice::SlicePattern;
use std::collections::HashMap;
use std::iter::{Map, Take};
use std::mem::size_of;
use std::slice::Iter;

use log::info;
use tokio::join;
use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends, Buffer, BindGroup, ComputePipeline, BufferSlice, MapMode, Device, Queue, SubmissionIndex, BufferView, Adapter};

use crate::chess::{GpuBoard, Side};
use crate::shaders::{Shader, self};
use crate::wgpu_util::SliceExtension;

const WORKGROUP_SIZE: u64 = 64;

pub struct GpuChessEvaluator {
    device: Device,
    buffers: BoardLists,
    queue: Queue,
    global_data: Buffer,
    just_zero: Buffer,
    out_index: Buffer,
    out_index_staging: Buffer,
    expand_shader: Shader
}

impl GpuChessEvaluator {
    /// Can only be called just after creation
    pub async fn set_input(&mut self, c: &BufferCombo, boards: impl IntoIterator<Item = GpuBoard>, to_move: Side, move_num: u32) {
        // self.in_buf.slice(..).map_buffer(&self.device, wgpu::MapMode::Write).await.unwrap();

        let mut i = 0;
        boards.into_iter().for_each(|board| {
            let start_write = i * size_of::<GpuBoard>();
            self.queue.write_buffer(self.buffers.get_in(c), start_write as u64, &board.to_bytes());
            i += 1;
        });

        assert!(move_num < 4);
        let mut data = [0; 12];
        data[0..4].copy_from_slice(bytemuck::bytes_of(&(i as u32)));
        data[4..8].copy_from_slice(bytemuck::bytes_of(&to_move.gpu_representation()));
        data[8..12].copy_from_slice(bytemuck::bytes_of(&move_num));
        self.queue.write_buffer(&self.global_data, 0, &data);
    }

    pub fn set_global_data(&self, to_move: Side, move_num: u32) {
        assert!(move_num < 4);
        let mut data = [0; 8];
        data[0..4].copy_from_slice(bytemuck::bytes_of(&to_move.gpu_representation()));
        data[4..8].copy_from_slice(bytemuck::bytes_of(&move_num));
        self.queue.write_buffer(&self.global_data, 4, &data);
    }

    pub fn run_expansion(&mut self, combo: &BufferCombo) -> SubmissionIndex {
        let mut command_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());
        let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass_encoder.set_pipeline(&self.expand_shader.1);
        pass_encoder.set_bind_group(0, &combo.bind, &[]);
        pass_encoder.dispatch_workgroups((self.buffers.boards_per_buf as f64 / WORKGROUP_SIZE as f64) as u32, 1, 1);
        drop(pass_encoder);
        command_encoder.copy_buffer_to_buffer(
            &self.out_index,
            0, // Source offset
            &self.global_data,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        command_encoder.copy_buffer_to_buffer(
            &self.out_index,
            0, // Source offset
            &self.out_index_staging,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        command_encoder.copy_buffer_to_buffer(
            &self.just_zero,
            0, // Source offset
            &self.out_index,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        return self.queue.submit([command_encoder.finish()]);
    }

    pub async fn get_output<'a>(&'a self, combo: &BufferCombo) -> ChessOutputBufferView {
        let mut command_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());
        command_encoder.copy_buffer_to_buffer(
            &self.buffers.get_out(combo),
            0, // Source offset
            &self.buffers.staging,
            0, // Destination offset
            self.buffers.buffer_size,
        );
        self.queue.submit([command_encoder.finish()]);

        self.buffers.staging().slice(..).map_buffer(&self.device, wgpu::MapMode::Read).await.unwrap();
        self.out_index_staging.slice(..).map_buffer(&self.device, wgpu::MapMode::Read).await.unwrap();
        
        let staging_view = self.buffers.staging().slice(..).get_mapped_range();
        let out_index_view = self.out_index_staging.slice(..).get_mapped_range();

        let amount: u32 = *bytemuck::from_bytes(&out_index_view.as_slice());
        drop(out_index_view);
        self.out_index_staging.unmap();
        return ChessOutputBufferView{ buf_view: Some(staging_view), amount: amount as usize, buf: &self.buffers.staging()};
    }

    pub fn create_combo(&self, input: u8, output: u8) -> BufferCombo {
        return self.buffers.create_combo(input, output, self);
    }
}

pub struct ChessOutputBufferView<'a> {
    buf_view: Option<BufferView<'a>>,
    amount: usize,
    buf: &'a Buffer
}

impl ChessOutputBufferView<'_> {
    pub fn get_size(&self) -> usize {
        return self.amount;
    }

    pub fn iter(&self) -> impl Iterator<Item = GpuBoard> + '_ {
        // Safety: buf_view is always Some at this point
        let chunks = self.buf_view.as_ref().unwrap().as_chunks::<{size_of::<GpuBoard>()}>().0;
        let iter = chunks.iter().take(self.amount).map(|b| GpuBoard::from_bytes(*b));
        return iter;
    }
}

impl Drop for ChessOutputBufferView<'_> {
    fn drop(&mut self) {
        let view = self.buf_view.take();
        drop(view);
        self.buf.unmap();
    }
}

pub async fn init_gpu_evaluator(adapter: &Adapter) -> GpuChessEvaluator {
    info!("Using gpu adapter: {:?}", adapter.get_info());

    let (device, queue) = adapter.request_device(&DeviceDescriptor::default(), None).await.expect("Failed to open GPU");

    let global_data = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Global Data Uniform"),
            size: 3 * size_of::<u32>() as u64, 
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

    let buffers = BoardLists::init(&device);

    let expand_shader = shaders::expand_pipeline(&device);

    return GpuChessEvaluator { device, buffers, queue, global_data, just_zero, out_index, out_index_staging, expand_shader };
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

pub struct BoardLists {
    buffers: [Buffer; 4],
    staging: Buffer,
    /// The amount of boards per buffer (for buffers that contain boards)
    boards_per_buf: u64,
    /// The size of buffers (for buffers that contain boards) (in bytes)
    buffer_size: u64,
    combo_cache: HashMap<(u8, u8), BufferCombo>,
}

impl BoardLists {
    pub fn init(device: &Device) -> Self {
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
            boards_per_buf = 128;
        }
        let buffer_size = boards_per_buf * size_of::<GpuBoard>() as u64;
        info!("We're allocating buffers of size {buffer_size}, which fits {boards_per_buf} boards");

        let buffers = std::array::from_fn(|i| {
            device.create_buffer(
                &BufferDescriptor { 
                    label: Some(&format!("Board Storage Buf #{i}")),
                    size: buffer_size, 
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false
                }
            )
        });

        let staging = device.create_buffer(
            &BufferDescriptor { 
                label: Some(&format!("Board Storage Staging")),
                size: buffer_size, 
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                mapped_at_creation: false
            }
        );

        return Self {buffers, boards_per_buf, buffer_size, staging, combo_cache: HashMap::default()};
    }

    pub fn create_combo(&self, input: u8, output: u8, engine: &GpuChessEvaluator) -> BufferCombo {
        let bind_group = engine.device.create_bind_group(
            &BindGroupDescriptor {
                label: None,
                layout: &engine.expand_shader.0,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(self.buffers[input as usize].as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(self.buffers[output as usize].as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Buffer(engine.out_index.as_entire_buffer_binding())
                    },
                ]
            }
        );

        return BufferCombo {
            bind: bind_group,
            input: input,
            output: output,
        };
    }

    pub fn get_in(&self, combo: &BufferCombo) -> &Buffer {
        return &self.buffers[combo.input as usize];
    }

    pub fn get_out(&self, combo: &BufferCombo) -> &Buffer {
        return &self.buffers[combo.output as usize];
    }

    pub fn staging(&self) -> &Buffer {
        &self.staging
    }
}

pub struct BufferCombo {
    input: u8,
    output: u8,
    bind: BindGroup,
}