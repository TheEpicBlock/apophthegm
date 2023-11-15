const King = 1u;
const Queen = 2u;
const Bishop = 3u;
const Rook = 4u;
const Horsy = 5u;
const Pawn = 6u;

struct Board {
  pieces: array<u32, 10>
}

struct GlobalData {
  input_size: u32,
  to_move: u32,
  move_index: u32,
}

fn getPiece(board: ptr<function, Board>, x: u32, y: u32) -> u32 {
  return ((*board).pieces[y] >> (x * 4u)) & 0xFu;
}