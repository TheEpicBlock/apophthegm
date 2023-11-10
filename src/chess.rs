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
        let mut fen = str.chars();

        {
            let pieces = fen.by_ref().take_while(|c| *c != ' ');
            let mut y = 0;
            let mut x = 0;
            for piece_char in pieces {
                if piece_char == '/' {
                    y += 1;
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
        return self.pieces[loc.0 as usize];
    }

    pub fn set(&mut self, loc: Location, piece: Option<Piece>) {
        self.pieces[loc.0 as usize] = piece;
    }

    pub fn write(&self, bytes: &mut [u8]) {
        for y in 0..8u8 {
            for x_seg in 0..4u8 {
                let p1 = self.get(Location::new(x_seg*2, y)).map_or(0, |p| p.as_nibble()) << 4;
                let p2 = self.get(Location::new(x_seg*2+1, y)).map_or(0, |p| p.as_nibble());
                bytes[(4*y + x_seg) as usize] = p1 | p2;
            }
        }
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

    fn from_letters(x: char, y: char) -> Self {
        assert!(x.is_ascii_lowercase());
        assert!(y.is_ascii_digit());
        Self::new(x.to_ascii_char().unwrap().as_byte() - b'a', y.to_ascii_char().unwrap().as_byte() - b'0')
    }

    fn get_x(&self) -> u8 {
        self.0 >> 3
    }

    fn x_as_char(&self) -> ascii::Char {
        (b'A' + self.get_x()).as_ascii().unwrap()
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

    fn from_fen_char(char: char) -> Self {
        let side = if char.is_ascii_uppercase() { Side::White } else { Side::Black };
        let piece = match char.to_ascii_uppercase() {
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

    pub fn as_nibble(&self) -> u8 {
        let side_indicator = match self.side {
            Side::Black => 0x0,
            Side::White => 0x8,
        };
        return side_indicator | (self.ty as u8);
    }
}

#[derive(Clone, Copy, Enum)]
pub enum Side {
    Black,
    White
}

#[derive(Clone, Copy)]
pub enum PieceType {
    King = 1,
    Queen = 2,
    Bishop = 3,
    Rook = 4,
    Horsy = 5,
    Pawn = 6
}