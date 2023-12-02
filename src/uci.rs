use std::{io, sync::{atomic::{AtomicBool, AtomicU16, AtomicU64}, Mutex, Arc}, rc::Rc};

use pollster::FutureExt;

use crate::chess::{GameState, Move, EvalScore, Side};

pub fn start_loop(mut engine: impl EngineComs) -> ! {
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
    let mut current_search = None;
    

    loop {
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        let mut cmd = buffer.split_ascii_whitespace();
        match cmd.next() {
            Some("position") => {
                let Some(pos_type) = cmd.next() else { panic!("Invalid position command") };
                let fen;
                if pos_type == "startpos" {
                    fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_owned();
                } else {
                    fen = cmd.by_ref().take(6).collect::<Vec<_>>().join(" ");
                }

                let mut state = GameState::from_fen(&fen);

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
                if (&gamestate).is_none() {
                    panic!("Can't search if you don't give me a position D:");
                }

                let coms = Arc::new(UciEvalSession {
                    to_move: gamestate.as_ref().unwrap().to_move,
                    stopped: AtomicBool::new(false),
                    depth: AtomicU16::new(0),
                    nodes: AtomicU64::new(0),
                    best: Mutex::new(None),
                });
                current_search = Some(coms.clone());

                engine.start_session(coms.clone(), gamestate.clone().unwrap()).block_on();
            }
            Some("stop") => {
                let Some(ref coms) = current_search else { panic!("no active search") };
                coms.stop();
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

pub struct UciEvalSession {
    to_move: Side,
    stopped: AtomicBool,
    depth: AtomicU16,
    nodes: AtomicU64,
    best: Mutex<Option<Move>>
}

impl UciEvalSession {
    pub fn set_best(&self, m: Move, score: EvalScore) {
        if self.is_stopped() {
            return;
        }
        *self.best.lock().unwrap() = Some(m);
        let depth = self.depth.load(std::sync::atomic::Ordering::Relaxed);
        let nodes = self.nodes.load(std::sync::atomic::Ordering::Relaxed);
        println!("info score cp {} depth {depth} nodes {nodes} pv {m}", score.centipawn_relative(self.to_move));
    }

    pub fn report_depth_and_nodes(&self, depth: u16, nodes: u64) {
        self.depth.fetch_max(depth, std::sync::atomic::Ordering::Relaxed);
        let n = self.nodes.fetch_add(nodes, std::sync::atomic::Ordering::Relaxed);
        println!("info depth {} nodes {}", depth, n+nodes);
    }

    pub fn stop(&self) {
        if self.is_stopped() {
            return;
        }
        self.stopped.store(true, std::sync::atomic::Ordering::SeqCst);

        let best = *self.best.lock().unwrap();
        match best {
            None => {
                // TODO what should we do here?
                println!("bestmove 0000");
            }
            Some(x) => {
                println!("bestmove {}", x);
            },
        }
    }

    pub fn is_stopped(&self) -> bool {
        return self.stopped.load(std::sync::atomic::Ordering::SeqCst);
    }
}

pub trait EngineComs {
    async fn start_session(&mut self, coms: Arc<UciEvalSession>, state: GameState);
}