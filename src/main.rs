#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(slice_as_chunks)]
#![feature(slice_pattern)]
#![allow(unused_imports)]
#![allow(dead_code)]


mod chess;

use core::slice::SlicePattern;
use std::{mem::size_of, thread, time::Duration};

use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends};

use crate::chess::{GameState, GpuBoard, board::convert};

const BOARDS_IN_BUF: u64 = 1048*1048;
const WORKGROUP_SIZE: u64 = 64;
const BUFFER_SIZE: u64 = size_of::<GpuBoard>() as u64 * BOARDS_IN_BUF;

#[tokio::main]
async fn main() {
    env_logger::init();
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
    // let adapter = instance.enumerate_adapters(Backends::GL).next().expect("WebGPU no does work :(");
    println!("{:?}", adapter.get_info());

    let (device, queue) = adapter.request_device(&DeviceDescriptor::default(), None).await.expect("Failed to open GPU");

    device.start_capture();

    let board = GameState::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    println!("Start:\n{}", board.get_board());

    let in_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Input"),
            size: BUFFER_SIZE, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST, 
            mapped_at_creation: true
        }
    );
    // board.write(&mut in_buf.slice(..).get_mapped_range_mut());
    in_buf.slice(0..32).get_mapped_range_mut().copy_from_slice(&convert::<GpuBoard>(&board.get_board()).to_bytes());
    in_buf.unmap();

    let out_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Output"),
            size: BUFFER_SIZE, 
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
    input_size.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::bytes_of(&1u32));
    input_size.unmap();

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
            size: BUFFER_SIZE, 
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

    {
        let mut command_encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
        let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass_encoder.set_pipeline(&pipeline);
        pass_encoder.set_bind_group(0, &bind_group, &[]);
        pass_encoder.dispatch_workgroups(1000, 1, 1);
        drop(pass_encoder);
        command_encoder.copy_buffer_to_buffer(
            &out_buf,
            0, // Source offset
            &in_buf,
            0, // Destination offset
            BUFFER_SIZE,
        );
        command_encoder.copy_buffer_to_buffer(
            &out_index,
            0, // Source offset
            &input_size,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        command_encoder.copy_buffer_to_buffer(
            &just_zero,
            0, // Source offset
            &out_index,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        queue.submit([command_encoder.finish()]);
    }

    println!("TWOOO:");

    let mut command_encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
    pass_encoder.set_pipeline(&pipeline);
    pass_encoder.set_bind_group(0, &bind_group, &[]);
    pass_encoder.dispatch_workgroups(ceil_div(BOARDS_IN_BUF, WORKGROUP_SIZE), 1, 1);
    drop(pass_encoder);
    command_encoder.copy_buffer_to_buffer(
        &out_buf,
        0, // Source offset
        &staging_buf,
        0, // Destination offset
        BUFFER_SIZE,
    );
    command_encoder.copy_buffer_to_buffer(
        &out_index,
        0, // Source offset
        &out_index_staging,
        0, // Destination offset
        1 * size_of::<u32>() as u64,
    );
    queue.submit([command_encoder.finish()]);


    staging_buf
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| {});
    out_index_staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| {});
    device.poll(wgpu::Maintain::Wait); // TODO: poll in the background instead of blocking
    let amount: u32 = *bytemuck::from_bytes(out_index_staging.slice(0..4).get_mapped_range().as_slice());
    let s = &staging_buf.slice(..).get_mapped_range();
    // s.as_chunks::<{size_of::<u32>()}>().0.iter().for_each(|b| {
    //     let board = u32::from_le_bytes(*b);
    //     println!("{board:#034b}");
    // });
    println!("{}", amount);
    s.as_chunks::<{size_of::<GpuBoard>()}>().0.iter().take(amount as usize + 1).for_each(|b| {
        let board = GpuBoard::from_bytes(*b);
        println!("{board}");
    });
    device.stop_capture();

    // thread::sleep(Duration::from_secs(32));
}

fn ceil_div(a: u64, b: u64) -> u32 {
    (a as f64 / b as f64).ceil() as u32
}