struct Board {
  pieces: array<u32, 8>
}

@group(0) @binding(0)
var<storage, read_write> input: array<Board>;
@group(0) @binding(1)
var<storage, read_write> output: array<Board>;
@group(0) @binding(2)
var<uniform> input_size: u32;
@group(0) @binding(3)
var<storage, read_write> out_index: atomic<u32>;

const King = 1u;
const Queen = 2u;
const Bishop = 3u;
const Rook = 4u;
const Horsy = 5u;
const Pawn = 6u;

@compute @workgroup_size(64)
fn main(
  @builtin(global_invocation_id)
  global_id : vec3u,

  @builtin(local_invocation_id)
  local_id : vec3u,
) {
  // Avoid accessing the buffer out of bounds
  if (global_id.x >= input_size) {
    return;
  }
  var board = input[global_id.x];
  var to_move = 0x8u;
  if (input_size > 1u) {
    to_move = 0u;
  }

  var pawn_start_rank = 6u; // 0-indexed!
  if (to_move == 0x8u) {
    pawn_start_rank = 1u; // 0-indexed!
  }

  var pawn_promote_rank = 1u; // 0-indexed!
  if (to_move == 0x8u) {
    pawn_promote_rank = 6u; // 0-indexed!
  }


  for (var x = 0u; x < 8u; x++) {
    for (var y = 0u; y < 8u; y++) {
      // Pieces are nibbles
      let piece = getPiece(&board, x, y);
      if ((piece & 0x8u) == to_move) {
        if ((piece & 0x7u) == Pawn) {
          // Pawn
          let offset = ((to_move >> 3u) * 2u) - 1u;
          // Upward movement
          if (getPiece(&board, x, y+offset) == 0u) {
            if (y == pawn_promote_rank) {//0-indexed
              // Promote
              var new_board = board;
              let clear_mask = ~(0xFu << (x*4u));
              new_board.pieces[y] &= clear_mask; // Remove the pawn
              let yNew = y+offset;
              // Promote various
              let out = atomicAdd(&out_index, 4u);
              new_board.pieces[yNew] &= clear_mask;
              new_board.pieces[yNew] |= (Queen << (x*4u));
              output[out] = new_board;
              new_board.pieces[yNew] &= clear_mask;
              new_board.pieces[yNew] |= (Bishop << (x*4u));
              output[out+1u] = new_board;
              new_board.pieces[yNew] &= clear_mask;
              new_board.pieces[yNew] |= (Horsy << (x*4u));
              output[out+2u] = new_board;
              new_board.pieces[yNew] &= clear_mask;
              new_board.pieces[yNew] |= (Rook << (x*4u));
              output[out+3u] = new_board;
            } else {
              // Regular upwards move
              var new_board = movePiece(&board, piece, x, y, x, y+offset);
              let out = atomicAdd(&out_index, 1u);
              output[out] = new_board;

              if (y == pawn_start_rank && getPiece(&board, x, y+(offset*2u)) == 0u) {
                var new_board2 = movePiece(&board, piece, x, y, x, y+(offset*2u));
                let out = atomicAdd(&out_index, 1u);
                output[out] = new_board2;
              }
            }
          }
          // Capture
          if (x + 1u < 8u && isOpponent(&board, to_move, x + 1u, y+offset)) {
            var new_board = movePiece(&board, piece, x, y, x + 1u, y+offset);
            let out = atomicAdd(&out_index, 1u);
            output[out] = new_board;
          }
          // Yes, this check is still correct, because it's unsigned
          if (x - 1u < 8u && isOpponent(&board, to_move, x - 1u, y+offset)) {
            var new_board = movePiece(&board, piece, x, y, x - 1u, y+offset);
            let out = atomicAdd(&out_index, 1u);
            output[out] = new_board;
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