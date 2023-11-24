use bytemuck::{Pod, Zeroable};
use wgpu::{Device, PipelineLayoutDescriptor, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, ComputePipeline, include_wgsl, ShaderModuleDescriptor, BindGroupLayout, BindGroup, BindGroupDescriptor, BindGroupEntry, DynamicOffset};

use crate::{gpu::{GpuGlobalData, GpuAllocations}, buffers::AllocToken, chess::{GpuBoard, EvalScore}};

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

pub struct ExpansionBindGroupMngr {
    
}

pub struct ExpansionBuffers<'a> {
    pub input: &'a AllocToken<GpuBoard>,
    pub output: &'a AllocToken<GpuBoard>,
}

impl ExpansionBindGroupMngr {
    pub fn create(engine: &GpuGlobalData, alloc: &GpuAllocations, buffers: ExpansionBuffers) -> BindOut<2> {
        let expansion_bind = engine.device.create_bind_group(
            &BindGroupDescriptor {
                label: None,
                layout: &engine.expand_shader.0,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(buffers.input.buffer(&alloc.boards).as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(buffers.output.buffer(&alloc.boards).as_entire_buffer_binding())
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
        let o = BuffOffsets {
            buf_offset_0: buffers.input.start_elem(),
            buf_offset_1: buffers.output.start_elem(),
            buf_offset_2: 0,
            buf_offset_3: 0,
        };
        return BindOut(expansion_bind, o);
    }
}

pub struct EvalContractBindGroupMngr {
    
}

pub struct EvalContractBuffers<'a> {
    pub parent_evals_boards: &'a AllocToken<EvalScore>,
    pub child_boards: &'a AllocToken<GpuBoard>,
}

impl EvalContractBindGroupMngr {
    pub fn create(engine: &GpuGlobalData, alloc: &GpuAllocations, buffers: EvalContractBuffers) -> BindOut<2> {
        let expansion_bind = engine.device.create_bind_group(
            &BindGroupDescriptor {
                label: None,
                layout: &engine.eval_contract_shader.0,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(buffers.child_boards.buffer(&alloc.boards).as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(buffers.parent_evals_boards.buffer(&alloc.evals).as_entire_buffer_binding())
                    },
                ]
            }
        );
        let o = BuffOffsets {
            buf_offset_0: 0,
            buf_offset_1: buffers.child_boards.start_elem(),
            buf_offset_2: buffers.parent_evals_boards.start_elem(),
            buf_offset_3: 0,
        };
        return BindOut(expansion_bind, o);
    }
}

pub struct ContractBindGroupMngr {
    
}

pub struct ContractBuffers<'a> {
    pub parent_evals_boards: &'a AllocToken<EvalScore>,
    pub child_boards: &'a AllocToken<GpuBoard>,
}

impl ContractBindGroupMngr {
    pub fn create(engine: &GpuGlobalData, alloc: &GpuAllocations, buffers: ContractBuffers) -> BindOut<2> {
        let expansion_bind = engine.device.create_bind_group(
            &BindGroupDescriptor {
                label: None,
                layout: &engine.contract_shader.0,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(buffers.child_boards.buffer(&alloc.boards).as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(buffers.parent_evals_boards.buffer(&alloc.evals).as_entire_buffer_binding())
                    },
                ]
            }
        );
        let o = BuffOffsets {
            buf_offset_0: 0,
            buf_offset_1: buffers.child_boards.start_elem(),
            buf_offset_2: buffers.parent_evals_boards.start_elem(),
            buf_offset_3: 0,
        };
        return BindOut(expansion_bind, o);
    }
}

pub struct FillMaxBindGroupMngr {
    
}

pub struct FillMaxBuffers<'a> {
    pub boards: &'a AllocToken<EvalScore>,
}

impl FillMaxBindGroupMngr {
    pub fn create(engine: &GpuGlobalData, alloc: &GpuAllocations, buffers: FillMaxBuffers) -> BindOut<2> {
        let expansion_bind = engine.device.create_bind_group(
            &BindGroupDescriptor {
                label: None,
                layout: &engine.fill_max_shader.0,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(engine.global_data.as_entire_buffer_binding())
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(buffers.boards.buffer(&alloc.evals).as_entire_buffer_binding())
                    },
                ]
            }
        );
        let o = BuffOffsets {
            buf_offset_0: 0,
            buf_offset_1: buffers.boards.start_elem(),
            buf_offset_2: 0,
            buf_offset_3: 0,
        };
        return BindOut(expansion_bind, o);
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct BuffOffsets {
    buf_offset_0: u32,
    buf_offset_1: u32,
    buf_offset_2: u32,
    buf_offset_3: u32,
}

pub struct BindOut<const SIZE: usize>(pub BindGroup, pub BuffOffsets);