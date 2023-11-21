use wgpu::{Device, PipelineLayoutDescriptor, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, ComputePipeline, include_wgsl, ShaderModuleDescriptor, BindGroupLayout};

pub const WORKGROUP_SIZE: u64 = 64;

macro_rules! include_shader {
    ($($token:tt)*) => {
        ShaderModuleDescriptor {
            label: Some($($token)*),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(include_str!("lib.wgsl"), include_str!($($token)*))))
        }
    };
}

pub struct Shader(pub BindGroupLayout, pub ComputePipeline);

pub fn expand(device: &Device) -> Shader {
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

    let pipeline = device.create_compute_pipeline(
        &wgpu::ComputePipelineDescriptor {
            label: Some("Expand"),
            layout: Some(&device.create_pipeline_layout(
                &PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[]
                }
            )),
            module: &device.create_shader_module(include_shader!("expand.wgsl")),
            entry_point: "expansion_pass"
        }
    );

    return Shader(bind_group_layout, pipeline);
}

pub fn eval_contract(device: &Device) -> Shader {
    let bind_group_layout = device.create_bind_group_layout(
        &BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
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

    let pipeline = device.create_compute_pipeline(
        &wgpu::ComputePipelineDescriptor {
            label: Some("Eval Contract"),
            layout: Some(&device.create_pipeline_layout(
                &PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[]
                }
            )),
            module: &device.create_shader_module(include_shader!("eval_contract.wgsl")),
            entry_point: "eval_contract_pass"
        }
    );

    return Shader(bind_group_layout, pipeline);
}

pub fn contract(device: &Device) -> Shader {
    let bind_group_layout = device.create_bind_group_layout(
        &BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
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

    let pipeline = device.create_compute_pipeline(
        &wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&device.create_pipeline_layout(
                &PipelineLayoutDescriptor {
                    label: Some("Contract"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[]
                }
            )),
            module: &device.create_shader_module(include_shader!("contract.wgsl")),
            entry_point: "eval_contract_pass"
        }
    );

    return Shader(bind_group_layout, pipeline);
}

pub fn fill_max(device: &Device) -> Shader {
    let bind_group_layout = device.create_bind_group_layout(
        &BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
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
            ],
        }
    );

    let pipeline = device.create_compute_pipeline(
        &wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&device.create_pipeline_layout(
                &PipelineLayoutDescriptor {
                    label: Some("Fill 0xFFFF"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[]
                }
            )),
            module: &device.create_shader_module(include_shader!("fill_max.wgsl")),
            entry_point: "fill_pass"
        }
    );

    return Shader(bind_group_layout, pipeline);
}