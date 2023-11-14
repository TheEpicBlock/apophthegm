use std::fmt;

use crate::{gpu::{GpuChessEvaluator, init_gpu_evaluator}, chess::{StandardBoard, GameState}};

use super::{Board, board::convert, GpuBoard};

trait TestEngine {
    type Out;
    async fn get_moves(board_in: GameState) -> Vec<Self::Out>;
}

struct GpuTester;

impl TestEngine for GpuTester {
    type Out = GpuBoard;
    async fn get_moves(board_in: GameState) -> Vec<Self::Out> {
        let mut engine = init_gpu_evaluator().await;
        engine.set_input([convert(&board_in.get_board())]).await;
        engine.run_pass(true);
        let out = engine.get_output().await;
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