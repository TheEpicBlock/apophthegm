use std::fmt::{Write, Debug};
use std::ops::{Index, IndexMut};
use std::{fmt::Display, ascii, mem::size_of, default};
use bytemuck::{NoUninit, Pod, Zeroable};
use enum_map::enum_map;

use crate::buffers::BufferData;

use super::piece::PieceExt;
use super::{Location, Piece, Side, PieceType, Move};

pub trait Board: Display {
    fn new_empty() -> Self;

    fn get(&self, index: Location) -> Option<Piece>;

    fn set(&mut self, index: Location, piece: Option<Piece>);

    fn is_valid(&self, last_moved: Side) -> bool {
        let mut kings = enum_map!{ _ => 0};

        for loc in Location::all() {
            if let Some(king) = self.get(loc).get_as(PieceType::King) {
                kings[king.side] += 1;
                if king.side == last_moved {
                    // The king that last moved cannot be in check
                    let horizontal_dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                    for d in horizontal_dirs {
                        for i in 1.. {
                            if let Some(nloc) = loc.try_add(d.0 * i, d.1 * i) {
                                if let Some(p) = self.get(nloc) {
                                    if p.side == king.side {
                                        break;
                                    } else if p.ty == PieceType::Rook || p.ty == PieceType::Queen {
                                        return false;
                                    } else if i == 1 && p.ty == PieceType::King {
                                        return false;
                                    } else {
                                        break;
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                    }
    
                    let horizonotal_dirs = [(1, 1), (-1, 1), (-1, -1), (1, -1)];
                    for d in horizonotal_dirs {
                        for i in 1.. {
                            if let Some(nloc) = loc.try_add(d.0 * i, d.1 * i) {
                                if let Some(p) = self.get(nloc) {
                                    if p.side == king.side {
                                        break;
                                    } else if p.ty == PieceType::Bishop || p.ty == PieceType::Queen {
                                        return false;
                                    } else if i == 1 && p.ty == PieceType::King {
                                        return false;
                                    } else {
                                        break;
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                    }
    
                    let horse_pos = [(2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (-1, 2), (1, -2), (-1, 2)];
                    for dloc in horse_pos {
                        if let Some(nloc) = loc.try_add(dloc.0, dloc.1) {
                            if let Some(horse) = self.get(nloc).get_as(PieceType::Horsy) {
                                if horse.side != king.side {
                                    return false;
                                }
                            }
                        }
                    }

                    let dy = if king.side == Side::Black { -1 } else { 1 };
                    if let Some(nloc) = loc.try_add(1, dy) {
                        if let Some(pawn) = self.get(nloc).get_as(PieceType::Pawn) {
                            if pawn.side != king.side {
                                return false;
                            }
                        }
                    }
                    if let Some(nloc) = loc.try_add(-1, dy) {
                        if let Some(pawn) = self.get(nloc).get_as(PieceType::Pawn) {
                            if pawn.side != king.side {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        if kings[Side::White] != 1 {
            return false;
        }
        if kings[Side::Black] != 1 {
            return false;
        }

        return true;
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
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
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuBoard([u32; 9]);

impl PartialEq<Self> for GpuBoard {
    fn eq(&self, other: &Self) -> bool {
        self.0[0..(8*4)] == other.0[0..(8*4)]
    }
}

impl BufferData for GpuBoard {
    const SIZE: usize = size_of::<Self>();
}

impl Board for GpuBoard {
    fn new_empty() -> Self {
        Self([0; 9])
    }

    fn get(&self, index: Location) -> Option<Piece> {
        let row = u32::from_le(self.0[index.get_y() as usize]);
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
        let mut row = u32::from_le(self.0[index.get_y() as usize]);
        let nibble: u32 = match piece {
            None => 0x00,
            Some(x) => x.as_nibble() as u32
        };

        row &= !(0x0F << (index.get_x() * 4));
        row |= nibble << (index.get_x() * 4);
        self.0[index.get_y() as usize] = row.to_le();
    }
}

impl GpuBoard {
    // Used for debugging
    pub fn get_prev(&self) -> usize {
        return u32::from_le(self.0[8 as usize]) as usize;
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

    let mut promote = None;
    if before.get(start_pos).unwrap().ty == PieceType::Pawn && (end_pos.get_y() == 0 || end_pos.get_y() == 7) {
        promote = Some(after.get(end_pos).unwrap().ty);
    }

    return Ok(Move(start_pos, end_pos, promote));
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

impl Debug for GpuBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)?;
        Ok(())
    }
}
impl Debug for StandardBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
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
    use crate::chess::{Location, Move, Piece, Side, PieceType, board::{StandardBoard, find_move}, GameState};

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

    #[test]
    fn find_move_normal() {
        let mut a = StandardBoard::new_empty();
        let mut b = StandardBoard::new_empty();
        let piece = Piece::new(Side::White, PieceType::Pawn);
        a.set(Location::new(0, 1), Some(piece));
        b.set(Location::new(0, 3), Some(piece));

        assert_eq!(find_move(&a, &b), Ok(Move(Location::new(0, 1), Location::new(0, 3), None)));
    }

    #[test]
    fn find_move_last_rank() {
        // Make sure that the bot doesn't think that everything that moves to the last rank is doing a promotion
        let mut a = StandardBoard::new_empty();
        let mut b = StandardBoard::new_empty();
        let piece = Piece::new(Side::White, PieceType::Queen);
        a.set(Location::new(0, 6), Some(piece));
        b.set(Location::new(0, 7), Some(piece));

        assert_eq!(find_move(&a, &b), Ok(Move(Location::new(0, 6), Location::new(0, 7), None)));
    }

    #[test]
    fn board_valid() {
        fn check(str: &str, exp: bool) {
            let state = GameState::from_fen(str);
            assert_eq!(state.get_board().is_valid(state.to_move.opposite()), exp);
        }
        check("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", true);
        // Not enough kings
        check("8/8/8/8/8/8/k7/8 w KQkq - 0 1", false);
        check("8/8/8/8/8/8/k7/8 b KQkq - 0 1", false);
        
        // White to move, and it should move out of check
        check("7k/2r5/8/8/8/2K5/8/8 w - - 0 1", true);
        // Black to move, and black can now capture the king which is illegal
        check("7k/2r5/8/8/8/2K5/8/8 b - - 0 1", false);

        check("8/5k2/8/2p5/1K6/8/8/8 b - - 0 1", false); // Black can capture with pawn
        check("8/5k2/8/8/1K3q2/8/8/8 b - - 0 1", false); // Black can capture with queen
        check("8/5k2/3q4/8/1K6/8/8/8 b - - 0 1", false); // Black can capture with queen
        check("8/5k2/3b4/8/1K6/8/8/8 b - - 0 1", false); // Black can capture with bishop
        check("8/5k2/8/8/1K3Q2/8/8/8 w - - 0 1", false); // White can capture with queen
        check("8/8/5k2/3N4/1K6/8/8/8 w - - 0 1", false); // White can capture with horse

        check("8/4P3/5k2/8/1K6/8/8/8 w - - 0 1", true); // White can *not* capture with pawn
        check("8/1kp3R1/8/8/8/8/8/7K w - - 0 1", true); // White can *not* capture with the rook, a black pawn is in the way
        check("8/1kP3R1/8/8/8/8/8/7K w - - 0 1", true); // White can *not* capture with the rook, a white pawn is in the way
        
    }
}