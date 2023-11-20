
use enum_map::{EnumMap, enum_map};
use crate::chess::board::Board;

use super::{Location, board::StandardBoard, Piece, Side, Move, PieceType};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GameState {
    pieces: StandardBoard,
    pub to_move: Side,
    en_passant_sq: Option<Location>,
    castles: EnumMap<Side, Castles>
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

    pub fn play(&mut self, m: Move) {
        self.to_move = self.to_move.opposite();
        let prev = self.get(m.0);

        if let Some(king) = self.get(m.0) && king.ty == PieceType::King {
            if u8::abs_diff(m.0.get_x(), m.1.get_x()) == 2 {
                // Castling!
                let castle_state = &mut self.castles[king.side];
                if m.1.get_x() < m.0.get_x() {
                    castle_state.queenside = false;
                    self.set(m.0.with_x(0), None);
                    self.set(m.0.with_x(3), Some(Piece { ty: PieceType::Rook, side: king.side }));
                } else {
                    castle_state.kingside = false;
                    self.set(m.0.with_x(7), None);
                    self.set(m.0.with_x(5), Some(Piece { ty: PieceType::Rook, side: king.side }));
                }
            }
        }

        let old_en_passant_sq = self.en_passant_sq;
        self.en_passant_sq = None;

        if let Some(piece) = prev && piece.ty == PieceType::Pawn {
            if Some(m.1) == old_en_passant_sq {
                // En-passant was played, need to remove the capture pawn
                let captured_location = match piece.side {
                    Side::Black => old_en_passant_sq.unwrap() + (0, 1),
                    Side::White => old_en_passant_sq.unwrap() + (0, -1),
                };
                self.set(captured_location, None);
            }
            if u8::abs_diff(m.0.get_y(), m.1.get_y()) == 2 {
                // A pawn was moved from the starting rank, need to update the en-passant square
                match piece.side {
                    Side::Black => self.en_passant_sq = Some(m.0 + (0, -1)),
                    Side::White => self.en_passant_sq = Some(m.0 + (0, 1)),
                }
            }
        }

        self.set(m.0, None);
        self.set(m.1, prev);
        if let Some(promotion) = m.2 {
            if let Some(prev_piece) = prev {
                self.set(m.1, Some(Piece { ty: promotion, side: prev_piece.side}));
            }
        }
    }
}

#[cfg(test)]
mod test {

    use crate::chess::Move;

    use super::GameState;

    #[test]
    fn play_normal_move() {
        let mut state = GameState::from_fen("8/8/8/8/8/8/1K5k/8 w - - 0 1");
        state.play(Move::from_str("b2c2"));
        assert_eq!(state, GameState::from_fen("8/8/8/8/8/8/2K4k/8 b - - 0 1"));
    }

    #[test]
    fn play_promotion() {
        let mut state = GameState::from_fen("8/1P6/8/8/8/8/5K1k/8 w - - 0 1");
        state.play(Move::from_str("b7b8N"));
        assert_eq!(state, GameState::from_fen("1N6/8/8/8/8/8/5K1k/8 b - - 0 1"));
    }

    #[test]
    fn play_promotion2() {
        let mut state = GameState::from_fen("8/1P6/8/8/8/8/5K1k/8 w - - 0 1");
        state.play(Move::from_str("b7b8Q"));
        assert_eq!(state, GameState::from_fen("1Q6/8/8/8/8/8/5K1k/8 b - - 0 1"));
    }

    // Testcases for Chess960
    // #[test]
    // fn play_castle_white_short() {
    //     let mut state = GameState::from_fen("rnbqk2r/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQK2R w KQkq - 0 1");
    //     state.play(Move::from_str("e1h1"));
    //     assert_eq!(state, GameState::from_fen("rnbqk2r/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQ1RK1 b Qkq - 0 1"));
    // }

    // #[test]
    // fn play_castle_black_short() {
    //     let mut state = GameState::from_fen("rnbqk2r/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQK2R b KQkq - 0 1");
    //     state.play(Move::from_str("e8h8"));
    //     assert_eq!(state, GameState::from_fen("rnbq1rk1/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQK2R w KQq - 0 1"));
    // }

    // #[test]
    // fn play_castle_white_long() {
    //     let mut state = GameState::from_fen("r3kbnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/R3KBNR w KQkq - 0 1");
    //     state.play(Move::from_str("e1a1"));
    //     assert_eq!(state, GameState::from_fen("r3kbnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/2KR1BNR b Kkq - 0 1"));
    // }

    // #[test]
    // fn play_castle_black_long() {
    //     let mut state = GameState::from_fen("r3kbnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/2KR1BNR b Kkq - 0 1");
    //     state.play(Move::from_str("e8a8"));
    //     assert_eq!(state, GameState::from_fen("2kr1bnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/2KR1BNR w Kk - 0 1"));
    // }
    
    #[test]
    fn play_castle_white_short() {
        let mut state = GameState::from_fen("rnbqk2r/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQK2R w KQkq - 0 1");
        state.play(Move::from_str("e1g1"));
        assert_eq!(state, GameState::from_fen("rnbqk2r/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQ1RK1 b Qkq - 0 1"));
    }

    #[test]
    fn play_castle_black_short() {
        let mut state = GameState::from_fen("rnbqk2r/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQK2R b KQkq - 0 1");
        state.play(Move::from_str("e8g8"));
        assert_eq!(state, GameState::from_fen("rnbq1rk1/pppp1ppp/7n/4p3/1b2P3/3B1N2/PPPP1PPP/RNBQK2R w KQq - 0 1"));
    }

    #[test]
    fn play_castle_white_long() {
        let mut state = GameState::from_fen("r3kbnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/R3KBNR w KQkq - 0 1");
        state.play(Move::from_str("e1c1"));
        assert_eq!(state, GameState::from_fen("r3kbnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/2KR1BNR b Kkq - 0 1"));
    }

    #[test]
    fn play_castle_black_long() {
        let mut state = GameState::from_fen("r3kbnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/2KR1BNR b Kkq - 0 1");
        state.play(Move::from_str("e8c8"));
        assert_eq!(state, GameState::from_fen("2kr1bnr/ppp1qppp/n2p4/4p3/4P1Q1/2NPB3/PPP2PPP/2KR1BNR w Kk - 0 1"));
    }

    #[test]
    fn play_en_passant() {
        let mut state = GameState::from_fen("8/8/8/8/2p5/8/1P3K1k/8 w - - 0 1");
        state.play(Move::from_str("b2b4"));
        assert_eq!(state, GameState::from_fen("8/8/8/8/1Pp5/8/5K1k/8 b - b3 0 1"));
        state.play(Move::from_str("c4b3"));
        assert_eq!(state, GameState::from_fen("8/8/8/8/8/1p6/5K1k/8 w - - 0 2"));
    }
}