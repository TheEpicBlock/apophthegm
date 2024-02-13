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
#![feature(test)]

mod chess;
mod gpu;
mod gpu_tree;
mod buffers;
pub(crate) mod misc;
mod shaders;
mod uci;

use core::slice::SlicePattern;
use std::{mem::size_of, thread, time::Duration, rc::Rc, sync::Arc, cell::RefCell};

use chess::{EvalScore, Board, MAX_MOVES};
use float_ord::FloatOrd;
use gpu::{init_gpu_evaluator, GpuGlobalData, GpuAllocations};
use gpu_tree::GpuTree;
use tokio::{runtime::Handle, sync::mpsc::Sender};
use tokio_util::task::LocalPoolHandle;
use uci::{EngineComs, UciEvalSession};
use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends};

use crate::{chess::{GameState, GpuBoard, board::{convert, self}, Side}, gpu::init_adapter};

const BOARDS_IN_BUF: u64 = 1048*1048;
const WORKGROUP_SIZE: u64 = 64;
const BUFFER_SIZE: u64 = size_of::<GpuBoard>() as u64 * BOARDS_IN_BUF;

#[tokio::main]
async fn main() {
    env_logger::init();
    let engine_coms = start();
    uci::start_loop(engine_coms);
}

fn start() -> impl EngineComs {
    let thread = LocalPoolHandle::new(1);
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<(Arc<UciEvalSession>, GameState)>(1);
    thread.spawn_pinned(|| {async move {
        let adapter = init_adapter().await;
        let engine = init_gpu_evaluator(&adapter).await;
        let mut allocations = GpuAllocations::init(engine.device.clone());

        loop {
            let Some((coms, state)) = receiver.recv().await else {break;};
            engine.device.start_capture();
            let mut tree = GpuTree::new(&engine, &mut allocations);

            tree.init_layer_from_state(&state);
            tree.expand_last_layer().await;
            
            let first_moves: Vec<_> = tree.view_boards_last()
                .await
                .cast_t()
                .iter()
                .filter(|m| m.is_valid(state.to_move))
                .map(|b| b.clone())
                .collect();
            drop(tree);
            
            let mut trees: Vec<_> = first_moves.iter().map(|m| {
                let mut tree = GpuTree::new(&engine, &allocations);
                tree.init_layer(&[*m], state.to_move.opposite());
                tree
            }).collect();


            let mut best_per_move: Vec<_> = first_moves.iter().map(|_| {
                EvalScore::worst(state.to_move)
            }).collect();

            let mut best_score = EvalScore::worst(state.to_move);
            for (i, tree) in trees.iter_mut().enumerate() {
                loop {
                    if allocations.fits(tree.last_layer().size() * MAX_MOVES) {
                        tree.expand_last_layer().await;
                        coms.report_depth_and_nodes(tree.last_layer().depth() as u16, tree.last_layer().size() as u64);
                    } else {
                        break;
                    }
                }

                tree.contract_all().await;

                let result = tree.view_evals(0).await.cast_t()[0];
                if EvalScore::better(&result, &best_score, state.to_move).is_ge() {
                    let board = first_moves[i];
                    coms.set_best(board::find_move(&state.get_board(), &board).unwrap(), result);
                    best_score = result;
                }
                if EvalScore::better(&result, &best_per_move[i], state.to_move).is_gt() {
                    best_per_move[i] = result;
                }
                // tree.remove_above(1);
                // tree.filter_last_layer(result).await;
                // tree.shrink_last_layer(3);
            }
            // for (i, tree) in trees.iter_mut().enumerate() {
            //     loop {
            //         if allocations.fits(tree.last_layer().size() * MAX_MOVES) {
            //             tree.expand_last_layer().await;
            //             coms.report_depth_and_nodes(tree.last_layer().depth() as u16, tree.last_layer().size() as u64);
            //         } else {
            //             break;
            //         }
            //     }

            //     tree.contract_all().await;

            //     let result = tree.view_evals(0).await.cast_t()[0];
            //     if EvalScore::better(&result, &best_score, state.to_move).is_ge() {
            //         let board = first_moves[i];
            //         coms.set_best(board::find_move(&state.get_board(), &board).unwrap(), result);
            //         best_score = result;
            //     }
            //     if EvalScore::better(&result, &best_per_move[i], state.to_move).is_gt() {
            //         best_per_move[i] = result;
            //     }
            // }
            engine.device.stop_capture();
            coms.stop();
        }
    }});

    Coms {
        sender
    }
}

struct Coms {
    sender: Sender<(Arc<UciEvalSession>, GameState)>
}

impl EngineComs for Coms {
    async fn start_session(&mut self, coms: Arc<UciEvalSession>, state: GameState) {
        self.sender.send((coms, state)).await.unwrap();
    }
}

#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    env_logger::init();
}