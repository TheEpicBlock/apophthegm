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

use chess::EvalScore;
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

    // let board = GameState::from_fen("8/5P2/8/8/6k1/8/1pQ3K1/8 w - - 0 1");
    // let mut engine = init_gpu_evaluator(&init_adapter().await).await;
    // let pass_1 = engine.create_combo(0, 1);
    // let pass_2 = engine.create_combo(1, 2);
    // let pass_3 = engine.create_combo(2, 3);
    // engine.set_input(&pass_1, [convert(&board.get_board())]).await;
    // engine.run_expansion(&pass_1, Side::White).await;
    // engine.run_expansion(&pass_2, Side::Black).await;
    // engine.run_expansion(&pass_3, Side::White).await;
    // engine.run_eval_contract(&pass_3, Side::White, 0).await;
    // engine.run_contract(&pass_2, Side::Black, 0).await;
    // let bout = engine.get_output_boards(&pass_1).await;
    // let eout = engine.get_output_evals(&pass_2).await;
    // Iterator::zip(bout.iter(), eout.iter()).for_each(|(b, e)| {
    //     println!("{b}=={}", e.to_centipawn());
    // });
                    

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

                let start_side = state.to_move;

                let pass_1 = engine.create_combo(0, 1);
                let pass_2 = engine.create_combo(1, 2);
                let pass_3 = engine.create_combo(2, 3);
                engine.set_input(&pass_1, [convert(&state.get_board())]).await;
                engine.run_expansion(&pass_1, start_side).await;

                let first_boards = engine.get_output_boards(&pass_1).await.iter().collect::<Vec<_>>();
                coms.report_depth_and_nodes(1, first_boards.len() as u64);
                let mut best_score = EvalScore::worst(start_side);
                for b in first_boards {
                    engine.set_input(&pass_1, [b]).await;
                    engine.run_expansion(&pass_1, start_side.opposite()).await;
                    coms.report_depth_and_nodes(2, engine.get_out_boards_len(&pass_1));

                    engine.run_expansion(&pass_2, start_side).await;
                    let num_boards = engine.get_out_boards_len(&pass_2);
                    coms.report_depth_and_nodes(3, num_boards);

                    if num_boards * 218 <= engine.boards_per_buf() {
                        // third pass!
                        engine.run_expansion(&pass_3, start_side.opposite()).await;
                        coms.report_depth_and_nodes(4, engine.get_out_boards_len(&pass_3));

                        engine.run_eval_contract(&pass_3, start_side.opposite(), 0).await;
                        engine.run_contract(&pass_2, start_side, 0).await;
                    } else {
                        engine.run_eval_contract(&pass_2, start_side, 0).await;
                    }
    
                    let bout = engine.get_output_boards(&pass_1).await;
                    let eout = engine.get_output_evals(&pass_2).await;
                    let (_best_board, score) = Iterator::zip(bout.iter(), eout.iter()).max_by(|a, b| EvalScore::better(&a.1, &b.1, start_side.opposite())).unwrap();
                    if EvalScore::better(&score, &best_score, start_side).is_gt() {
                        let best_move = board::find_move(&state.get_board(), &b).unwrap();
                        best_score = score;
                        coms.set_best(best_move, score.to_centipawn());
                    }
                }
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