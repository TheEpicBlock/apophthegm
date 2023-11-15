use std::fmt;

use wgpu::Adapter;
use pollster::FutureExt as _;

use crate::{gpu::{GpuChessEvaluator, init_gpu_evaluator, init_adapter}, chess::{StandardBoard, GameState}};

use super::{Board, board::convert, GpuBoard};

// Only create an adapter once, to ensure no threading issues present themselves
#[ctor::ctor]
static GPU_ADAPTER: Adapter = {
    init_adapter().block_on()
};

trait TestEngine {
    type Out;
    async fn get_moves(board_in: GameState) -> Vec<Self::Out>;
}

struct GpuTester;

impl TestEngine for GpuTester {
    type Out = GpuBoard;
    async fn get_moves(board_in: GameState) -> Vec<Self::Out> {
        let mut engine = init_gpu_evaluator(&GPU_ADAPTER).await;
        let buf_combo = engine.create_combo(0, 1);
        engine.set_input(&buf_combo, [convert(&board_in.get_board())], super::Side::White, 0).await;
        engine.run_expansion(&buf_combo).await;
        let out = engine.get_output(&buf_combo).await;
        out.iter().collect()
    }
}

async fn assert_moves(start: &'static str, expected_moves: &[&'static str]) {
    let start_board = GameState::from_fen(start);
    let res = GpuTester::get_moves(start_board).await;
    let expected_boards: Vec<_> = expected_moves.iter().map(|i| convert(&GameState::from_fen(i).get_board())).collect();
    let mut err_str = String::new();
    for i in &expected_boards {
        if !res.contains(i) {
            fmt::write(&mut err_str, format_args!("Expected the following board, but it wasn't present: \n{i}")).unwrap();
        }
    }

    for i in &res {
        if !expected_boards.contains(i) {
            fmt::write(&mut err_str, format_args!("The following board shouldn't be a valid move: \n{i}")).unwrap();
        }
    }

    if !err_str.is_empty() {
        panic!("{}", err_str);
    }
}

#[tokio::test]
async fn pawn_basic() {
    assert_moves(
        "8/8/8/8/8/P/8/8 w KQkq - 0 1",
        &[
            "8/8/8/8/P/8/8/8 b KQkq - 0 1"
        ]
    ).await;
}

#[tokio::test]
async fn pawn_double() {
    assert_moves(
        "8/8/8/8/8/8/P/8 w KQkq - 0 1",
        &[
            "8/8/8/8/8/P/8/8 b KQkq - 0 1",
            "8/8/8/8/P/8/8/8 b KQkq - 0 1"
        ]
    ).await;
}

#[tokio::test]
async fn pawn_block() {
    assert_moves(
        "8/8/8/8/8/n/P/8 w KQkq - 0 1",
        &[
        ]
    ).await;
}

#[tokio::test]
async fn pawn_capture() {
    assert_moves(
        "8/8/8/8/1n6/P/8/8 w KQkq - 0 1",
        &[
            "8/8/8/8/1P6/8/8/8 b KQkq - 0 1",
            "8/8/8/8/Pn6/8/8/8 b KQkq - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn pawn_promote() {
    assert_moves(
        "8/P7/8/8/8/8/8/8 w KQkq - 0 1",
        &[
            "Q/8/8/8/8/8/8/8 b KQkq - 0 1",
            "R/8/8/8/8/8/8/8 b KQkq - 0 1",
            "B/8/8/8/8/8/8/8 b KQkq - 0 1",
            "N/8/8/8/8/8/8/8 b KQkq - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn pawn_capture_promote() {
    assert_moves(
        "nr6/P/8/8/8/8/8/8 w KQkq - 0 1",
        &[
            "nQ6/8/8/8/8/8/8/8 b KQkq - 0 1",
            "nR6/8/8/8/8/8/8/8 b KQkq - 0 1",
            "nB6/8/8/8/8/8/8/8 b KQkq - 0 1",
            "nN6/8/8/8/8/8/8/8 b KQkq - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn horse() {
    assert_moves(
        "8/8/8/3N4/8/8/8/8 w - - 0 1",
        &[
            "8/8/8/8/8/2N5/8/8 b - - 0 1",
            "8/8/8/8/8/4N3/8/8 b - - 0 1",
            "8/8/8/8/5N2/8/8/8 b - - 0 1",
            "8/8/5N2/8/8/8/8/8 b - - 0 1",
            "8/4N3/8/8/8/8/8/8 b - - 0 1",
            "8/2N5/8/8/8/8/8/8 b - - 0 1",
            "8/8/1N6/8/8/8/8/8 b - - 0 1",
            "8/8/8/8/1N6/8/8/8 b - - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn horse_corner() {
    assert_moves(
        "8/8/8/8/8/8/8/N7 w - - 0 1",
        &[
            "8/8/8/8/8/1N6/8/8 b - - 0 1",
            "8/8/8/8/8/8/2N5/8 b - - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn horse_capture() {
    assert_moves(
        "8/8/8/8/8/1p6/8/N7 b - - 0 1",
        &[
            "8/8/8/8/8/1N6/8/8 b - - 0 1",
            "8/8/8/8/8/1p6/2N5/8 b - - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn rook() {
    assert_moves(
        "8/8/8/2n5/8/2R3b1/8/8 w - - 0 1",
        &[
            "8/8/8/2n5/2R5/6b1/8/8 b - - 0 1",
            "8/8/8/2n5/8/1R4b1/8/8 b - - 0 1",
            "8/8/8/2n5/8/R5b1/8/8 b - - 0 1",
            "8/8/8/2n5/8/6b1/2R5/8 b - - 0 1",
            "8/8/8/2n5/8/6b1/8/2R5 b - - 0 1",
            "8/8/8/2n5/8/3R2b1/8/8 b - - 0 1",
            "8/8/8/2n5/8/4R1b1/8/8 b - - 0 1",
            "8/8/8/2n5/8/5Rb1/8/8 b - - 0 1",
            "8/8/8/2n5/8/6R1/8/8 b - - 0 1",
            "8/8/8/2R5/8/6b1/8/8 b - - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn bischop() {
    assert_moves(
        "8/8/8/8/B7/8/8/3r4 w - - 0 1",
        &[
            "8/8/8/1B6/8/8/8/3r4 b - - 0 1",
            "8/8/2B5/8/8/8/8/3r4 b - - 0 1",
            "8/3B4/8/8/8/8/8/3r4 b - - 0 1",
            "4B3/8/8/8/8/8/8/3r4 b - - 0 1",
            "8/8/8/8/8/1B6/8/3r4 b - - 0 1",
            "8/8/8/8/8/8/2B5/3r4 b - - 0 1",
            "8/8/8/8/8/8/8/3B4 b - - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn queen() {
    assert_moves(
        "8/8/2b5/4p3/8/2Q5/8/8 b - - 0 1",
        &[
            "8/8/2b5/4p3/8/1Q6/8/8 b - - 0 1",
            "8/8/2b5/4p3/8/Q7/8/8 b - - 0 1",
            "8/8/2b5/4p3/8/8/1Q6/8 b - - 0 1",
            "8/8/2b5/4p3/8/8/8/Q7 b - - 0 1",
            "8/8/2b5/4p3/8/8/2Q5/8 b - - 0 1",
            "8/8/2b5/4p3/8/8/8/2Q5 b - - 0 1",
            "8/8/2b5/4p3/8/8/3Q4/8 b - - 0 1",
            "8/8/2b5/4p3/8/8/8/4Q3 b - - 0 1",
            "8/8/2b5/4p3/8/3Q4/8/8 b - - 0 1",
            "8/8/2b5/4p3/8/4Q3/8/8 b - - 0 1",
            "8/8/2b5/4p3/8/5Q2/8/8 b - - 0 1",
            "8/8/2b5/4p3/8/6Q1/8/8 b - - 0 1",
            "8/8/2b5/4p3/8/7Q/8/8 b - - 0 1",
            "8/8/2b5/4p3/1Q6/8/8/8 b - - 0 1",
            "8/8/2b5/Q3p3/8/8/8/8 b - - 0 1",
            "8/8/2b5/4p3/2Q5/8/8/8 b - - 0 1",
            "8/8/2b5/2Q1p3/8/8/8/8 b - - 0 1",
            "8/8/2Q5/4p3/8/8/8/8 b - - 0 1",
            "8/8/2b5/4p3/3Q4/8/8/8 b - - 0 1",
            "8/8/2b5/4Q3/8/8/8/8 b - - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn king() {
    assert_moves(
        "8/8/8/8/8/8/1K6/8 w KQkq - 0 1",
        &[
            "8/8/8/8/8/8/2K5/8 b KQkq - 0 1",
            "8/8/8/8/8/8/K7/8 b KQkq - 0 1",
            "8/8/8/8/8/8/8/K7 b KQkq - 0 1",
            "8/8/8/8/8/8/8/1K6 b KQkq - 0 1",
            "8/8/8/8/8/8/8/2K5 b KQkq - 0 1",
            "8/8/8/8/8/K7/8/8 b KQkq - 0 1",
            "8/8/8/8/8/1K6/8/8 b KQkq - 0 1",
            "8/8/8/8/8/2K5/8/8 b KQkq - 0 1",
        ]
    ).await;
}


#[tokio::test]
async fn king_blocked() {
    assert_moves(
        "8/8/8/8/8/2p6/1KP5/8 w KQkq - 0 1",
        &[
            "8/8/8/8/8/2p6/K1P5/8 b KQkq - 0 1",
            "8/8/8/8/8/2p6/2P5/K7 b KQkq - 0 1",
            "8/8/8/8/8/2p6/2P5/1K6 b KQkq - 0 1",
            "8/8/8/8/8/2p6/2P5/2K5 b KQkq - 0 1",
            "8/8/8/8/8/K1p5/2P5/8 b KQkq - 0 1",
            "8/8/8/8/8/1Kp5/2P5/8 b KQkq - 0 1",
            "8/8/8/8/8/2K5/2P5/8 b KQkq - 0 1",
        ]
    ).await;
}

#[tokio::test]
async fn king_corner() {
    assert_moves(
        "8/8/8/8/8/8/8/K7 w KQkq - 0 1",
        &[
            "8/8/8/8/8/8/8/1K6 b KQkq - 0 1",
            "8/8/8/8/8/8/K/8 b KQkq - 0 1",
            "8/8/8/8/8/8/1K6/8 b KQkq - 0 1",
        ]
    ).await;
}