struct Board {
  pieces: array<u32, 8>
}

@group(0) @binding(0)
var<storage, read> input: array<Board>;
@group(0) @binding(1)
var<storage, read_write> output: array<Board>;

@compute @workgroup_size(64)
fn main(
  @builtin(global_invocation_id)
  global_id : vec3u,

  @builtin(local_invocation_id)
  local_id : vec3u,
) {
  // Avoid accessing the buffer out of bounds
  if (global_id.x >= 1u) {
    return;
  }
  var board = input[global_id.x];
  let to_move = 0x8u;

  var pawn_start_rank = 6u; // 0-indexed!
  if (to_move == 0x8u) {
    pawn_start_rank = 1u; // 0-indexed!
  }


  var out = 0u;
  for (var x = 0u; x < 8u; x++) {
    for (var y = 0u; y < 8u; y++) {
      // Pieces are nibbles
      let piece = getPiece(&board, x, y);
      if ((piece & 0x8u) == to_move) {
        if ((piece & 0x7u) == 6u) {
          // Pawn
          let offset = ((to_move >> 3u) * 2u) - 1u;
          // Upward movement
          if (getPiece(&board, x, y+offset) == 0u) {
            var new_board = movePiece(&board, piece, x, y, x, y+offset);
            output[out] = new_board;
            out += 1u;

            if (y == pawn_start_rank && getPiece(&board, x, y+(offset*2u)) == 0u) {
              var new_board2 = movePiece(&board, piece, x, y, x, y+(offset*2u));
              output[out] = new_board2;
              out += 1u;
            }
          }
          // Capture
          if (x + 1u < 8u && isOpponent(&board, to_move, x + 1u, y+offset)) {
            var new_board = movePiece(&board, piece, x, y, x + 1u, y+offset);
            output[out] = new_board;
            out += 1u;
          }
          // Yes, this check is still correct, because it's unsigned
          if (x - 1u < 8u && isOpponent(&board, to_move, x - 1u, y+offset)) {
            var new_board = movePiece(&board, piece, x, y, x - 1u, y+offset);
            output[out] = new_board;
            out += 1u;
          }
        }
      }
    }
  }
}

fn getPiece(board: ptr<function, Board>, x: u32, y: u32) -> u32 {
  return ((*board).pieces[y] >> (x * 4u)) & 0xFu;
}

fn isColour(board: ptr<function, Board>, colour: u32, x: u32, y: u32) -> bool {
  let p = getPiece(board, x, y);
  return p != 0u && ((p | 0x8u) == colour);
}

fn isOpponent(board: ptr<function, Board>, to_move: u32, x: u32, y: u32) -> bool {
  let p = getPiece(board, x, y);
  return p != 0u && ((p | 0x8u) != to_move);
}

fn movePiece(board: ptr<function, Board>, piece: u32, x: u32, y: u32, xNew: u32, yNew: u32) -> Board {
  var new_board = *board;
  new_board.pieces[y] &= ~(0xFu << (x*4u));
  new_board.pieces[yNew] &= ~(0xFu << (xNew*4u));
  new_board.pieces[yNew] |= (piece << (xNew*4u));
  return new_board;
}