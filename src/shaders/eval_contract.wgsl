@group(0) @binding(0)
var<uniform> globals: GlobalData;
@group(0) @binding(1)
var<storage, read> child_boards: array<Board>;
@group(0) @binding(2)
var<storage, read_write> parent_evals: array<atomic<u32>>;

@compute @workgroup_size(64)
fn eval_contract_pass(
  @builtin(global_invocation_id)
  global_id : vec3u,

  @builtin(local_invocation_id)
  local_id : vec3u,
) {
  // Avoid accessing the buffer out of bounds
  if (global_id.x >= globals.input_size) {
    return;
  }
  var board = child_boards[global_id.x];
  let score = u32(evalPosition(&board)) ^ (1u<<31u);
  let prev_index = getPrev(&board, globals.move_index);

  switch globals.to_move {
    case 0x8u: {
      atomicMax(&parent_evals[prev_index], score);
    }
    case 0x0u: {
      atomicMin(&parent_evals[prev_index], score);
    }
    default: {}
  }
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