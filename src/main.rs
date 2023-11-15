#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(slice_as_chunks)]
#![feature(slice_pattern)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(impl_trait_in_assoc_type)]

mod chess;
mod gpu;
pub(crate) mod wgpu_util;
mod shaders;

use core::slice::SlicePattern;
use std::{mem::size_of, thread, time::Duration};

use gpu::init_gpu_evaluator;
use wgpu::{RequestAdapterOptions, DeviceDescriptor, BufferDescriptor, BufferUsages, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindGroupDescriptor, BindGroupLayout, BindGroupEntry, PipelineLayoutDescriptor, ShaderModule, ShaderModuleDescriptor, include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, Backends};

use crate::{chess::{GameState, GpuBoard, board::convert, Side}, gpu::init_adapter};

const BOARDS_IN_BUF: u64 = 1048*1048;
const WORKGROUP_SIZE: u64 = 64;
const BUFFER_SIZE: u64 = size_of::<GpuBoard>() as u64 * BOARDS_IN_BUF;

#[tokio::main]
async fn main() {
    env_logger::init();
    let adapter = init_adapter().await;
    let mut engine = init_gpu_evaluator(&adapter).await;

    let starter_board = GameState::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");

    println!("Start:\n{}", starter_board.get_board());

    let combo_1 = engine.create_combo(0, 1);
    let combo_2 = engine.create_combo(1, 2);

    engine.set_input(&combo_1, [convert(&starter_board.get_board())], Side::White, 0).await;
    engine.run_expansion(&combo_1);

    engine.set_global_data(Side::Black, 1);
    engine.run_expansion(&combo_2);

    let out = engine.get_output(&combo_2).await;
    println!("Found {} states", out.get_size());
    // out.iter().for_each(|b| {
    //     println!("{b}");
    // });
    drop(out);
}

#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    env_logger::init();
}