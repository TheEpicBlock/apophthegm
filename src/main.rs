#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(slice_as_chunks)]
#![feature(slice_pattern)]
#![feature(impl_trait_in_assoc_type)]
#![feature(never_type)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod chess;
mod gpu;
pub(crate) mod wgpu_util;
mod shaders;
mod uci;

use core::slice::SlicePattern;
use std::{mem::size_of, thread, time::Duration, rc::Rc, sync::Arc};

use float_ord::FloatOrd;
use gpu::{init_gpu_evaluator, GpuChessEvaluator};
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
            let mut engine = init_gpu_evaluator(&adapter).await;
            loop {
                if coms.is_stopped() {
                    break;
                }

                let pass_1 = engine.create_combo(0, 1);
                engine.set_input(&pass_1, [convert(&state.get_board())], Side::White, 0).await;
                engine.run_expansion(&pass_1).await;

                let pass_2 = engine.create_combo(1, 2);
                engine.set_global_data(Side::Black, 1);
                engine.run_expansion(&pass_2).await;


                let pass_3 = engine.create_combo(2, 3);
                engine.set_global_data(Side::White, 2);
                engine.run_expansion(&pass_3).await;

                engine.run_eval_contract(&pass_3, Side::White, 2).await;
                engine.run_contract(&pass_2, Side::Black, 1).await;

                let bout = engine.get_output_boards(&pass_1).await;
                let eout = engine.get_output_evals(&pass_2).await;
                let (best_board, score) = Iterator::zip(bout.iter(), eout.iter().map(|f| FloatOrd(f))).max_by_key(|b| b.1).unwrap();
                let best_move = board::find_move(&state.get_board(), &best_board).unwrap();
                coms.set_best(best_move, score.0);
                coms.stop();
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