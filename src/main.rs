#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(slice_as_chunks)]
#![feature(slice_pattern)]
#![feature(impl_trait_in_assoc_type)]
#![feature(never_type)]
#![feature(let_chains)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod chess;
mod gpu;
mod gpu_tree;
mod buffers;
pub(crate) mod misc;
mod shaders;
mod uci;

use core::slice::SlicePattern;
use std::{mem::size_of, thread, time::Duration, rc::Rc, sync::Arc, cell::RefCell};

use chess::{EvalScore, Board};
use float_ord::FloatOrd;
use gpu::{init_gpu_evaluator, GpuGlobalData, GpuAllocations};
use gpu_tree::GpuTree;
use tokio::runtime::Handle;
use tokio_util::task::LocalPoolHandle;
use uci::ThreadedEngine;
use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends};

use crate::{chess::{GameState, GpuBoard, board::{convert, self}, Side}, gpu::init_adapter};

const BOARDS_IN_BUF: u64 = 1048*1048;
const WORKGROUP_SIZE: u64 = 64;
const BUFFER_SIZE: u64 = size_of::<GpuBoard>() as u64 * BOARDS_IN_BUF;

#[tokio::main]
async fn main() {
    env_logger::init();
    uci::start_loop(MahEngine);
}

struct MahEngine;

impl ThreadedEngine for MahEngine {
    fn spawn_lookup(&self, coms: Arc<uci::UciCommunication>, state: GameState) {
        let pool = LocalPoolHandle::new(1);
        let task = pool.spawn_pinned(|| {async move {
            let adapter = init_adapter().await;
            let engine = init_gpu_evaluator(&adapter).await;
            let mut allocations = GpuAllocations::init(engine.device.clone());
            let mut tree = GpuTree::new(&engine, &mut allocations);

            tree.init_layer_from_state(state);

            loop {
                if coms.is_stopped() {
                    break;
                }
            }
        }});
        tokio::spawn(task);
    }
}

#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    env_logger::init();
}