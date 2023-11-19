use std::fmt::Write;
use std::ops::{Index, IndexMut};
use std::{fmt::Display, ascii, mem::size_of, default};

use super::{Location, Piece, Side, PieceType, Move};

pub trait Board: Display {
    fn new_empty() -> Self;

    fn get(&self, index: Location) -> Option<Piece>;

    fn set(&mut self, index: Location, piece: Option<Piece>);
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct StandardBoard([Option<Piece>; 64]);

impl Board for StandardBoard {
    fn new_empty() -> Self {
        Self([None; 64])
    }

    fn get(&self, index: Location) -> Option<Piece> {
        self.0[index.0 as usize]
    }

    fn set(&mut self, index: Location, piece: Option<Piece>) {
        self.0[index.0 as usize] = piece
    }
}

impl Index<Location> for StandardBoard {
    type Output = Option<Piece>;

    fn index(&self, index: Location) -> &Self::Output {
        &self.0[index.0 as usize]
    }
}

impl IndexMut<Location> for StandardBoard {
    fn index_mut(&mut self, index: Location) -> &mut Self::Output {
        &mut self.0[index.0 as usize]
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct GpuBoard([u8; 10*size_of::<u32>()]);

impl PartialEq<Self> for GpuBoard {
    fn eq(&self, other: &Self) -> bool {
        self.0[0..(8*4)] == other.0[0..(8*4)]
    }
}

impl Board for GpuBoard {
    fn new_empty() -> Self {
        Self([0; 10*size_of::<u32>()])
    }

    fn get(&self, index: Location) -> Option<Piece> {
        let row = u32::from_le_bytes(self.0.as_chunks::<{size_of::<u32>()}>().0[index.get_y() as usize]);
        let nibble = ((row >> (index.get_x() as usize * size_of::<u32>()) & 0x0F)) as u8;
        if nibble == 0 {
            return None;
        }
        let side = match nibble & 0x08 {
            0x8 => Side::White,
            0x0 => Side::Black,
            _ => unreachable!(),
        };
        // Will panic if the 'side' flag is set to white, but the piece is null. This shouldn't happen
        let ty = PieceType::from_triplet(nibble & 0x07);
        return Some(Piece::new(side, ty));
    }

    fn set(&mut self, index: Location, piece: Option<Piece>) {
        let row_start = index.get_y() as usize * size_of::<u32>();
        let byte = &mut self.0[row_start + (index.get_x() / 2) as usize];
        let nibble: u8 = match piece {
            None => 0x00,
            Some(x) => x.as_nibble()
        };

        *byte &= !(0x0F << (index.get_x() % 2 * 4));
        *byte |= nibble << (index.get_x() % 2 * 4);
    }
}

impl GpuBoard {
    pub fn from_bytes(b: [u8; size_of::<Self>()]) -> Self {
        GpuBoard(b)
    }

    pub fn to_bytes(self) -> [u8; size_of::<Self>()] {
        self.0
    }

    pub fn getPrev(&self) -> usize {
        return u32::from_le_bytes(self.0.as_chunks::<{size_of::<u32>()}>().0[8 as usize]) as usize;
    }
}

pub fn convert<T: Board>(input: &impl Board) -> T {
    let mut out = T::new_empty();
    Location::all().for_each(|l| {
        out.set(l, input.get(l));
    });
    return out;
}

pub fn find_move(before: &impl Board, after: &impl Board) -> Result<Move, &'static str> {
    let mut start_pos = None;
    for pos in Location::all() {
        if before.get(pos).is_some() && after.get(pos).is_none() {
            start_pos = Some(pos);
            break;
        }
    }
    let Some(start_pos) = start_pos else { return Err("Couldn't find start"); };

    let mut end_pos = None;
    for pos in Location::all() {
        if let Some(after_piece) = after.get(pos) {
            if before.get(pos) != Some(after_piece) {
                end_pos = Some(pos);
                break;
            }
        }
    }
    let Some(end_pos) = end_pos else { return Err("Couldn't find end"); };

    return Ok(Move(start_pos, end_pos));
}

fn display(input: &impl Board, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for y in (0..8).into_iter().rev() {
        for x in 0..8 {
            f.write_char(input.get(Location::new(x, y)).map_or('.', |p| p.to_char()))?;
        }
        f.write_char('\n')?;
    }

    Ok(())
}

impl Display for GpuBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)?;
        Ok(())
    }
}
impl Display for StandardBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

#[cfg(test)]
mod test {
    use crate::chess::{Location, Piece, Side, PieceType};

    use super::{GpuBoard, Board};

    #[test]
    fn gpu_get_set() {
        let mut b = GpuBoard::new_empty();
        let piece = Piece::new(Side::White, PieceType::Bishop);
        b.set(Location::new(0, 0), Some(piece));
        assert_eq!(b.get(Location::new(0, 0)), Some(piece));
        b.set(Location::new(1, 0), Some(piece));
        assert_eq!(b.get(Location::new(1, 0)), Some(piece));
        b.set(Location::new(0, 0), None);
        assert_eq!(b.get(Location::new(0, 0)), None);
        assert_eq!(b.get(Location::new(1, 0)), Some(piece));
    }
}