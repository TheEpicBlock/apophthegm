
use enum_map::{EnumMap, enum_map};
use crate::chess::board::Board;

use super::{Location, board::StandardBoard, Piece, Side};

pub struct GameState {
    pub pieces: StandardBoard,
    to_move: Side,
    en_passant_sq: Option<Location>,
    castles: EnumMap<Side, Castles>
}

pub struct Castles {
    kingside: bool,
    queenside: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self { pieces: StandardBoard::new_empty(), to_move: Side::White, en_passant_sq: None, castles: enum_map! { _ => Castles { kingside: false, queenside: false }} }
    }
}

impl GameState {
    pub fn from_fen(str: &str) -> Self {
        let mut state = Self::default();
        let mut fen = str.chars();

        {
            let pieces = fen.by_ref().take_while(|c| *c != ' ');
            let mut y = 7;
            let mut x = 0;
            for piece_char in pieces {
                if piece_char == '/' {
                    y -= 1;
                    x = 0;
                    if y == 8 {
                        panic!("Fen has too many ranks");
                    }
                    continue;
                }
                if piece_char.is_ascii_digit() {
                    x += piece_char.as_ascii().unwrap().to_u8() - b'0';
                    continue;
                }
                state.set(Location::new(x, y), Some(Piece::from_fen_char(piece_char)));
                x += 1;
            }
        }

        {
            let active_colour: Vec<_> = fen.by_ref().take_while(|c| *c != ' ').collect();
            assert_eq!(active_colour.len(), 1);
            state.to_move = match active_colour[0] {
                'w' => Side::White,
                'b' => Side::Black,
                _ => panic!("Invalid side {}", active_colour[0])
            }
        }

        {
            let castles = fen.by_ref().take_while(|c| *c != ' ');
            for i in castles {
                if i == '-' { continue; }
                let side = if i.is_uppercase() { Side::White } else { Side::Black };
                if i.to_ascii_uppercase() == 'K' {
                    state.castles[side].kingside = true;
                } else {
                    state.castles[side].queenside = true;
                }
                // TODO check for invalid letters
            }
        }

        {
            let en_passant: Vec<_> = fen.by_ref().take_while(|c| *c != ' ').collect();
            if en_passant.len() > 1 {
                state.en_passant_sq = Some(Location::from_letters(en_passant[0], en_passant[1]));
            }
        }

        return state;
    }

    pub fn get(&self, loc: Location) -> Option<Piece> {
        return self.pieces[loc];
    }

    pub fn set(&mut self, loc: Location, piece: Option<Piece>) {
        self.pieces[loc] = piece;
    }

    pub fn get_board(&self) -> impl Board {
        self.pieces
    }
}

