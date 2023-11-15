pub mod state;
pub mod board;
pub mod piece;
#[cfg(test)]
pub mod test;

use std::fmt::Display;
use std::ascii;

use ::ascii::ToAsciiChar;
pub use state::GameState;
pub use piece::{Piece, PieceType, Side};
pub use board::{Board, GpuBoard, StandardBoard};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Location(u8);

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.x_as_char(), self.get_y()+1) // +1 because of 0-indexing
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
        (b'a' + self.get_x()).as_ascii().unwrap()
    }

    fn get_y(&self) -> u8 {
        self.0 & 0x07
    }

    pub fn all() -> impl Iterator<Item = Location> {
        (0..64).into_iter().map(|i| Location(i))
    }
}

// Todo, encode promotion
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Move(pub Location, pub Location);

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.0, self.1)
    }
}

impl Move {
    // Parses long algebraic notation, compliant with uci
    pub fn from_str(str: &str) -> Move {
        assert_eq!(str.len(), 4);
        let str: Vec<_> = str.chars().collect();
        return Move(Location::from_letters(str[0], str[1]), Location::from_letters(str[2], str[3]));
    }
}