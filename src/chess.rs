use std::{fmt::Display, ascii};

use ::ascii::{AsAsciiStr, AsciiChar, ToAsciiChar};
use enum_map::{EnumMap, Enum, enum_map};

pub struct GameState {
    pieces: [Option<Piece>; 64],
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
        Self { pieces: [None; 64], to_move: Side::White, en_passant_sq: None, castles: enum_map! { _ => Castles { kingside: false, queenside: false }} }
    }
}

impl GameState {
    pub fn from_fen(str: &str) -> Self {
        let mut state = Self::default();
        let mut ascii = str.as_ascii_str().expect("Fen should be valid ASCII").chars();

        {
            let pieces = ascii.by_ref().take_while(|c| *c != ' ');
            let mut y = 0;
            let mut x = 0;
            for piece_char in pieces {
                if piece_char == '/' {
                    y += 1;
                    x = 0;
                    if y == 8 {
                        panic!("Fen has too many ranks");
                    }
                }
                state.set(Location::new(x, y), Some(Piece::from_fen_char(piece_char)));
                x += 1;
            }
        }

        {
            let active_colour: Vec<_> = ascii.by_ref().take_while(|c| *c != ' ').collect();
            assert_eq!(active_colour.len(), 1);
            state.to_move = match active_colour[0].as_char() {
                'w' => Side::White,
                'b' => Side::Black,
                _ => panic!("Invalid side {}", active_colour[0])
            }
        }

        {
            let en_passant: Vec<_> = ascii.by_ref().take_while(|c| *c != ' ').collect();
            if en_passant.len() > 1 {
                state.en_passant_sq = Some(Location::from_letters(en_passant[0], en_passant[1]));
            }
        }

        {
            let castles = ascii.by_ref().take_while(|c| *c != ' ');
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
        

        return state;
    }

    pub fn get(&self, loc: Location) -> Option<Piece> {
        return self.pieces[loc.0 as usize];
    }

    pub fn set(&mut self, loc: Location, piece: Option<Piece>) {
        self.pieces[loc.0 as usize] = piece;
    }
}

#[derive(Clone, Copy)]
pub struct Location(u8);

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.x_as_char(), self.get_y())
    }
}

impl Location {
    pub fn new(x: u8, y: u8) -> Self {
        assert!(x < 8);
        assert!(y < 8);
        Self((x << 3) | (y & 0x07))
    }

    fn from_letters(x: AsciiChar, y: AsciiChar) -> Self {
        assert!(x.is_ascii_lowercase());
        assert!(y.is_ascii_digit());
        Self::new(x.as_byte() - b'a', y.as_byte() - b'0')
    }

    fn get_x(&self) -> u8 {
        self.0 >> 3
    }

    fn x_as_char(&self) -> AsciiChar {
        (b'A' + self.get_x()).to_ascii_char().unwrap()
    }

    fn get_y(&self) -> u8 {
        self.0 & 0x07
    }
}

#[derive(Clone, Copy)]
pub struct Piece {
    pub ty: PieceType,
    pub side: Side
}

impl Piece {
    pub fn new(side: Side, ty: PieceType) -> Self {
        Piece { ty, side }
    }

    fn from_fen_char(char: AsciiChar) -> Self {
        let side = if char.is_ascii_uppercase() { Side::White } else { Side::Black };
        let piece = match char.to_ascii_uppercase().as_char() {
            'P' => PieceType::Pawn,
            'N' => PieceType::Horsy,
            'B' => PieceType::Bishop,
            'R' => PieceType::Rook,
            'Q' => PieceType::Queen,
            'K' => PieceType::King,
            _ => panic!("Invalid Fen piece {}", char),
        };
        return Piece::new(side, piece);
    }
}

#[derive(Clone, Copy, Enum)]
pub enum Side {
    Black,
    White
}

#[derive(Clone, Copy)]
pub enum PieceType {
    King,
    Queen,
    Bishop,
    Rook,
    Horsy,
    Pawn
}