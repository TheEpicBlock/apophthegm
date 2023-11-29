const King = 1u;
const Queen = 2u;
const Bishop = 3u;
const Rook = 4u;
const Horsy = 5u;
const Pawn = 6u;

struct Board {
  pieces: array<u32, 9>
}

struct GlobalData {
  input_size: u32,
  to_move: u32,
  move_index: u32,
  buf_offset_0: u32,
  buf_offset_1: u32,
  buf_offset_2: u32,
  buf_offset_3: u32,
}

fn getPiece(board: ptr<function, Board>, x: u32, y: u32) -> u32 {
  return ((*board).pieces[y] >> (x * 4u)) & 0xFu;
}

fn getPrev(board: ptr<function, Board>, index: u32) -> u32 {
  return (*board).pieces[8];
}