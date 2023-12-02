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

fn evalPosition(board: ptr<function, Board>) -> i32 {
  var eval_score = i32(0);

  for (var x = 0u; x < 8u; x++) {
    for (var y = 0u; y < 8u; y++) {
      // Pieces are nibbles
      let piece = getPiece(board, x, y);
      var piece_score = i32(0);
      let piece_type = piece & 0x7u;
      if (piece_type == Pawn) {
        piece_score = 100;
        if ((y == 3u || y == 4u) && (x == 3u || x == 4u)) {
          piece_score = 150;
        }
      } else if (piece_type == Horsy || piece_type == Bishop) {
        if (y == 0u || y == 7u) {
          piece_score = 250; // encourage developing pieces
        } else {
          piece_score = 300;
        }
      } else if (piece_type == Rook) {
        piece_score = 500;
      } else if (piece_type == Queen) {
        piece_score = 900;
      } else if (piece_type == King) {
        piece_score = 100000;
      }
      
      if ((piece & 0x8u) == 0u) {
        // Piece is black
        piece_score *= -1;
      }
      eval_score += piece_score;
    }
  }
  return eval_score;
}