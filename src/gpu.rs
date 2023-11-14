use core::slice::SlicePattern;
use std::iter::{Map, Take};
use std::mem::size_of;
use std::slice::Iter;

use log::info;
use tokio::join;
use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends, Buffer, BindGroup, ComputePipeline, BufferSlice, MapMode, Device, Queue, SubmissionIndex, BufferView, Adapter};

use crate::chess::GpuBoard;
use crate::wgpu_util::SliceExtension;

const WORKGROUP_SIZE: u64 = 64;

pub struct GpuChessEvaluator {
    device: Device,
    queue: Queue,
    /// The amount of boards per buffer (for buffers that contain boards)
    boards_per_buf: u64,
    /// The size of buffers (for buffers that contain boards) (in bytes)
    buffer_size: u64,
    in_buf: Buffer,
    out_buf: Buffer,
    input_size: Buffer,
    just_zero: Buffer,
    out_index: Buffer,
    staging_buf: Buffer,
    out_index_staging: Buffer,
    bind_layout: BindGroupLayout,
    bind: BindGroup,
    pipeline: ComputePipeline
}

impl GpuChessEvaluator {
    /// Can only be called just after creation
    pub async fn set_input(&mut self, boards: impl IntoIterator<Item = GpuBoard>) {
        // self.in_buf.slice(..).map_buffer(&self.device, wgpu::MapMode::Write).await.unwrap();

        let mut view = self.in_buf.slice(..).get_mapped_range_mut();
        let mut i = 0;
        boards.into_iter().for_each(|board| {
            let start_write = i * size_of::<GpuBoard>();
            view[start_write..(start_write+32)].copy_from_slice(&board.to_bytes());
            i += 1;
        });
        drop(view);
        self.in_buf.unmap();

        // self.input_size.slice(..).map_buffer(&self.device, wgpu::MapMode::Write).await.unwrap();
        let mut view = self.input_size.slice(..).get_mapped_range_mut();
        view.copy_from_slice(bytemuck::bytes_of(&(i as u32)));
        drop(view);
        self.input_size.unmap();
    }

    pub fn run_pass(&mut self, read_out: bool) -> SubmissionIndex {
        let mut command_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());
        let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass_encoder.set_pipeline(&self.pipeline);
        pass_encoder.set_bind_group(0, &self.bind, &[]);
        pass_encoder.dispatch_workgroups((self.boards_per_buf as f64 / WORKGROUP_SIZE as f64) as u32, 1, 1);
        drop(pass_encoder);
        command_encoder.copy_buffer_to_buffer(
            &self.out_buf,
            0, // Source offset
            &self.in_buf,
            0, // Destination offset
            self.buffer_size,
        );
        command_encoder.copy_buffer_to_buffer(
            &self.out_index,
            0, // Source offset
            &self.input_size,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        if read_out {
            command_encoder.copy_buffer_to_buffer(
                &self.out_index,
                0, // Source offset
                &self.out_index_staging,
                0, // Destination offset
                1 * size_of::<u32>() as u64,
            );
            command_encoder.copy_buffer_to_buffer(
                &self.out_buf,
                0, // Source offset
                &self.staging_buf,
                0, // Destination offset
                self.buffer_size,
            );
        }
        command_encoder.copy_buffer_to_buffer(
            &self.just_zero,
            0, // Source offset
            &self.out_index,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        return self.queue.submit([command_encoder.finish()]);
    }

    pub async fn get_output<'a>(&'a self) -> ChessOutputBufferView {
        self.staging_buf.slice(..).map_buffer(&self.device, wgpu::MapMode::Read).await.unwrap();
        self.out_index_staging.slice(..).map_buffer(&self.device, wgpu::MapMode::Read).await.unwrap();
        
        let staging_view = self.staging_buf.slice(..).get_mapped_range();
        let out_index_view = self.out_index_staging.slice(..).get_mapped_range();

        let amount: u32 = *bytemuck::from_bytes(&out_index_view.as_slice());
        drop(out_index_view);
        self.out_index_staging.unmap();
        return ChessOutputBufferView{ buf_view: Some(staging_view), amount: amount as usize, buf: &self.staging_buf};
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

    // Buffer size calculations
    let max_buffer_size = u64::min(device.limits().max_buffer_size, device.limits().max_storage_buffer_binding_size as u64);
    let max_boards_per_buf = max_buffer_size / size_of::<GpuBoard>() as u64;
    info!("Max buffer size is {max_buffer_size}, which fits {max_boards_per_buf} boards");
    let max_dispatch = device.limits().max_compute_workgroups_per_dimension as u64;
    let max_boards_dispatch = max_dispatch * WORKGROUP_SIZE;
    info!("Max dispatch is {max_dispatch}, which fits {max_boards_dispatch} boards");
    let boards_per_buf = u64::min(max_boards_per_buf, max_boards_dispatch);
    let buffer_size = boards_per_buf * size_of::<GpuBoard>() as u64;
    info!("We're allocating buffers of size {buffer_size}, which fits {boards_per_buf} boards");

    let in_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Input"),
            size: buffer_size, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST, 
            mapped_at_creation: true
        }
    );
    
    let out_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Output"),
            size: buffer_size, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }
    );

    let input_size = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Input Size Uniform"),
            size: 1 * size_of::<u32>() as u64, 
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST, 
            mapped_at_creation: true
        }
    );

    let just_zero = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Output Index Atomic"),
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

    let staging_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("staging"),
            size: buffer_size, 
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
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

    let bind_group_layout = device.create_bind_group_layout(
        &BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
            ],
        }
    );
    let bind_group = device.create_bind_group(
        &BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(in_buf.as_entire_buffer_binding())
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(out_buf.as_entire_buffer_binding())
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(input_size.as_entire_buffer_binding())
                },
                BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(out_index.as_entire_buffer_binding())
                },
            ]
        }
    );

    let pipeline = device.create_compute_pipeline(
        &wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&device.create_pipeline_layout(
                &PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[]
                }
            )),
            module: &device.create_shader_module(include_wgsl!("shader.wgsl")),
            entry_point: "main"
        }
    );

    return GpuChessEvaluator { device, queue, in_buf, out_buf, input_size, just_zero, out_index, staging_buf, out_index_staging, bind_layout: bind_group_layout, bind: bind_group, pipeline, boards_per_buf, buffer_size };
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