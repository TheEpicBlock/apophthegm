#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(slice_as_chunks)]

mod chess;

use std::{mem::size_of, thread, time::Duration};

use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends};

#[tokio::main]
async fn main() {
    env_logger::init();
    thread::sleep(Duration::from_secs(3));
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

    let out_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("Output"),
            size: 1000 * size_of::<f32>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }
    );

    let staging_buf = device.create_buffer(
        &BufferDescriptor { 
            label: Some("staging"),
            size: 1000 * size_of::<f32>() as u64, 
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
                }
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
                    resource: wgpu::BindingResource::Buffer(out_buf.as_entire_buffer_binding())
                }
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

    let mut command_encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
    pass_encoder.set_pipeline(&pipeline);
    pass_encoder.set_bind_group(0, &bind_group, &[]);
    pass_encoder.dispatch_workgroups((1000f64 / 64f64).ceil() as u32, 1, 1);
    drop(pass_encoder);
    command_encoder.copy_buffer_to_buffer(
        &out_buf,
        0, // Source offset
        &staging_buf,
        0, // Destination offset
        1000 * size_of::<f32>() as u64,
    );
    queue.submit([command_encoder.finish()]);


    let (sender, receiver) = futures_channel::oneshot::channel();
    staging_buf
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::Maintain::Wait); // TODO: poll in the background instead of blocking
    receiver
        .await
        .expect("communication failed")
        .expect("buffer reading failed");
    let s = &staging_buf.slice(..).get_mapped_range();
    println!("{}", s.iter().count());
    s.as_chunks::<{size_of::<f32>()}>().0.iter().for_each(|b| {
        println!("{}", f32::from_le_bytes(*b));
    });
    device.stop_capture();

    thread::sleep(Duration::from_secs(32));
}