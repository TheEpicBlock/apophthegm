use std::fmt;

use wgpu::Adapter;
use pollster::FutureExt as _;

use crate::{gpu::{GpuGlobalData, init_gpu_evaluator, init_adapter, GpuAllocations}, chess::{StandardBoard, GameState, Side, EvalScore}, gpu_tree::GpuTree};

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
        let engine = init_gpu_evaluator(&GPU_ADAPTER).await;
        let mut allocator = GpuAllocations::init(engine.device.clone());
        let mut tree = GpuTree::new(&engine, &mut allocator);
        tree.init_layer_from_state(&board_in);
        tree.expand_last_layer().await;
        return tree.view_boards_last().await.cast_t().into_iter().map(|b| b.clone()).collect();
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
        "8/8/8/8/8/1p6/8/N7 w - - 0 1",
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
        "8/8/2b5/4p3/8/2Q5/8/8 w - - 0 1",
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

#[tokio::test]
async fn multiple_expansions() {
    let board = GameState::from_fen("8/p7/8/8/8/8/4P2/8 w KQkq - 0 1");
    let engine = init_gpu_evaluator(&GPU_ADAPTER).await;
    let mut allocator = GpuAllocations::init(engine.device.clone());
    let mut tree = GpuTree::new(&engine, &mut allocator);
    tree.init_layer_from_state(&board);
    tree.expand_last_layer().await;
    tree.expand_last_layer().await;

    let boards_len: usize = tree.view_boards_last().await.cast_t().iter().count();
    // assert_eq!(boards.len() as u64, engine.get_out_boards_len(&pass_2));
    assert_eq!(boards_len, 4);
}

#[tokio::test]
async fn test_eval() {
    // It should be obviously better to move the pawn two spots than just one
    let board = GameState::from_fen("8/p7/8/8/8/8/4P2/8 w KQkq - 0 1");
    let engine = init_gpu_evaluator(&GPU_ADAPTER).await;
    let mut allocator = GpuAllocations::init(engine.device.clone());
    let mut tree = GpuTree::new(&engine, &mut allocator);
    tree.init_layer_from_state(&board);
    tree.expand_last_layer().await;
    tree.expand_last_layer().await;
    tree.contract_eval(2).await;
    let evals: Vec<EvalScore> = tree.view_evals(1).await.cast_t().into_iter().map(|x| x.clone()).collect();
    assert_eq!(evals.len(), 2);
    assert_eq!(evals, [EvalScore::from(0), EvalScore::from(50)]); // Might change in the future
    let best = evals.iter().max_by(|a, b| EvalScore::better(a, b, Side::White)).unwrap();
    let worst = evals.iter().min_by(|a, b| EvalScore::better(a, b, Side::White)).unwrap();
    assert!(best.to_centipawn() > worst.to_centipawn());
    
    // Test contract
    tree.contract(1).await;
    let evals: Vec<_> = tree.view_evals(0).await.cast_t().into_iter().map(|x| x.clone()).collect();
    assert_eq!(evals.len(), 1);
    assert_eq!(evals[0], *best);
}