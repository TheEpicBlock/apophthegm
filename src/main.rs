#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(slice_as_chunks)]
#![feature(slice_pattern)]
#![feature(impl_trait_in_assoc_type)]
#![feature(never_type)]
#![feature(let_chains)]
#![feature(get_many_mut)]
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
            tree.init_layer_from_state(&state);
            tree.expand_last_layer().await;
            let first_moves = tree.view_boards_last().await.cast_t().to_vec();
            drop(tree);

            let mut best_score = EvalScore::worst(state.to_move);
            for m in first_moves {
                if coms.is_stopped() {
                    break;
                }
                if !m.is_valid(state.to_move) {
                    continue;
                }

                let mut tree = GpuTree::new(&engine, &mut allocations);
                tree.init_layer(&[m], state.to_move.opposite());
                tree.expand_last_layer().await;
                tree.expand_last_layer().await;
                tree.contract_eval(2).await;
                tree.contract(1).await;
                let result = tree.view_evals(0).await.cast_t()[0];
                if EvalScore::better(&result, &best_score, state.to_move).is_ge() {
                    coms.set_best(board::find_move(&state.get_board(), &m).unwrap(), result);
                    best_score = result;
                }
            }
            coms.stop();
        }});
        tokio::spawn(task);
    }
}

#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    env_logger::init();
}