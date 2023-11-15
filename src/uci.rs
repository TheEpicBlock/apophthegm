use std::{io, sync::{atomic::AtomicBool, Mutex, Arc}, rc::Rc};

use crate::chess::{GameState, Move};

pub fn start_loop(engine: impl ThreadedEngine) -> ! {
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

                let coms = Arc::new(UciCommunication {
                    stopped: AtomicBool::new(false),
                    best: Mutex::new(None),
                });
                current_search = Some(coms.clone());

                engine.spawn_lookup(coms.clone(), gamestate.clone().unwrap());
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

pub struct UciCommunication{
    stopped: AtomicBool,
    best: Mutex<Option<Move>>
}

impl UciCommunication {
    pub fn set_best(&self, m: Move, score: f32) {
        if self.is_stopped() {
            return;
        }
        *self.best.lock().unwrap() = Some(m);
        println!("info pv {}", m);
        println!("info score cp {}", score as u32);
    }

    pub fn stop(&self) {
        if self.is_stopped() {
            return;
        }
        self.stopped.store(true, std::sync::atomic::Ordering::Relaxed);

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
        return self.stopped.load(std::sync::atomic::Ordering::Relaxed);
    }
}

pub trait ThreadedEngine {
    fn spawn_lookup(&self, coms: Arc<UciCommunication>, state: GameState);
}