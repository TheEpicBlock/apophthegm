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

fn getPrev(board: ptr<function, Board>, index: u32) -> u32 {
  switch index {
    case 0u: {
      return (*board).pieces[8] & (0xFFFFu);
    }
    case 1u: {
      return ((*board).pieces[8] & (0xFFFFu << 16u)) >> 16u;
    }
    case 2u: {
      return (*board).pieces[9] & (0xFFFFu);
    }
    case 3u: {
      return ((*board).pieces[9] & (0xFFFFu << 16u)) >> 16u;
    }
    default: {
      return 0u;
    }
  } 
}