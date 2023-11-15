use std::io;

use crate::chess::{GameState, Move};

fn start_loop() -> ! {
    let mut buffer = String::new();
    let stdin = io::stdin();
    stdin.read_line(&mut buffer).unwrap();

    // First line!
    match buffer.split_ascii_whitespace().next() {
        Some("uci") => {
            println!("id name {} (version {})", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            println!("id author {}", env!("CARGO_PKG_AUTHORS"));
            println!("uciok :3");
        }
        _ => {
            panic!("apophthegm is supposed to be used with the universal chess interface");
        }
    }

    let mut gamestate = None;
    

    loop {
        stdin.read_line(&mut buffer).unwrap();
        let mut cmd = buffer.split_ascii_whitespace();
        match cmd.next() {
            Some("position") => {
                let Some(pos_type) = cmd.next() else { panic!("Invalid position command") };
                let fen;
                if pos_type == "startpos" {
                    fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
                } else {
                    let Some(f) = cmd.next() else { panic!("Invalid position command") };
                    fen = f;
                }

                let mut state = GameState::from_fen(fen);

                if matches!(cmd.next(), Some("moves")) {
                    for move_str in cmd {
                        let m = Move::from_str(move_str);
                        state.play(m);
                    } 
                }

                gamestate = Some(state);
            }
            Some("go") => {
                for sub_cmd in cmd {
                    match sub_cmd {
                        "ponder" => panic!("pondering not supported"),
                        "searchmoves" => panic!("searches cannot be restricted"),
                        "depth" => panic!("depth cannot be restricted"),
                        "nodes" => panic!("nodes cannot be restricted"),
                        "mate" => panic!("I won't, sorry"),
                        _ => {}
                    }
                }
                if gamestate.is_none() {
                    panic!("Can't search if you don't give me a position D:");
                }
            }
            Some("isready") => {
                println!("readyok");
            }
            _ => {
                continue;
            }
        }
    }
}