use std::fmt::Debug;

use enum_map::Enum;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub ty: PieceType,
    pub side: Side
}

impl Piece {
    pub fn new(side: Side, ty: PieceType) -> Self {
        Piece { ty, side }
    }

    pub fn from_fen_char(char: char) -> Self {
        let side = if char.is_ascii_uppercase() { Side::White } else { Side::Black };
        let piece = PieceType::from_char(char);
        return Piece::new(side, piece);
    }

    pub fn as_nibble(&self) -> u8 {
        let side_indicator = match self.side {
            Side::Black => 0x0,
            Side::White => 0x8,
        };
        return side_indicator | (self.ty as u8);
    }

    pub fn to_char(&self) -> char {
        let piece_char = self.ty.to_char();
        return match self.side {
            Side::Black => piece_char.to_ascii_lowercase(),
            Side::White => piece_char.to_ascii_uppercase(),
        };
    }
}

impl Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Piece({})", self.to_char())
    }
}

#[derive(Clone, Copy, Enum, PartialEq, Eq)]
pub enum Side {
    Black,
    White
}

impl Side {
    pub fn gpu_representation(&self) -> u32 {
        return match self {
            Side::Black => 0x0,
            Side::White => 0x8,
        };
    }

    pub fn opposite(&self) -> Self {
        return match self {
            Side::Black => Side::White,
            Side::White => Side::Black,
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PieceType {
    King = 1,
    Queen = 2,
    Bishop = 3,
    Rook = 4,
    Horsy = 5,
    Pawn = 6
}

impl PieceType {
    /// Parse from three bits
    pub fn from_triplet(b: u8) -> Self {
        debug_assert!(b <= 0x06);
        debug_assert!(b != 0);
        match b {
            1 => Self::King,
            2 => Self::Queen,
            3 => Self::Bishop,
            4 => Self::Rook,
            5 => Self::Horsy,
            6 => Self::Pawn,
            _ => unreachable!()
        }
    }

    pub fn to_byte(&self) -> u8 {
        match self {
            PieceType::King => 1,
            PieceType::Queen => 2,
            PieceType::Bishop => 3,
            PieceType::Rook => 4,
            PieceType::Horsy => 5,
            PieceType::Pawn => 6,
        }
    }

    pub fn to_char(&self) -> char {
        match self {
            PieceType::King => 'K',
            PieceType::Queen => 'Q',
            PieceType::Bishop => 'B',
            PieceType::Rook => 'R',
            PieceType::Horsy => 'N',
            PieceType::Pawn => 'P',
        }
    }

    pub fn from_char(char: char) -> Self {
        match char.to_ascii_uppercase() {
            'P' => PieceType::Pawn,
            'N' => PieceType::Horsy,
            'B' => PieceType::Bishop,
            'R' => PieceType::Rook,
            'Q' => PieceType::Queen,
            'K' => PieceType::King,
            _ => panic!("Invalid char {}", char),
        }
    }
}