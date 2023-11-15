struct GlobalData {
  input_size: u32,
  to_move: u32,
  move_index: u32,
}

@group(0) @binding(0)
var<uniform> globals: GlobalData;
@group(0) @binding(1)
var<storage, read_write> self_boards: array<Board>;
@group(0) @binding(2)
var<storage, read_write> out: array<atomic<u32>>;

const hashmap_capacity = 256u;

@compute @workgroup_size(64)
fn compute_eval_pass(
  @builtin(global_invocation_id)
  global_id : vec3u,

  @builtin(local_invocation_id)
  local_id : vec3u,
) {
  // Avoid accessing the buffer out of bounds
  if (global_id.x >= globals.input_size) {
    return;
  }
  var board = input[global_id.x];
  let score = evalPosition(board);
  let move = getMove(&board, globals.to_move);
  let prev_move = getMove(&board, globals.to_move-1);

  let board_hash = 0u;
  let i = board_hash % hashmap_capacity;
  loop {
    let old_
  }
}

fn getMove(board: ptr<function, Board>, index: u32) -> u32 {
  switch index {
    case 0u: {
      return (*board).pieces[8] & (0xFFu);
    }
    case 1u: {
      return ((*board).pieces[8] & (0xFFu << 16u)) >> 16u;
    }
    case 2u: {
      return (*board).pieces[9] & (0xFFu);
    }
    case 3u: {
      return ((*board).pieces[9] & (0xFFu << 16u)) >> 16u;
    }
    default: {
      return 0u;
    }
  } 
}

fn evalPosition(board: ptr<function, Board>) -> f32 {
  var eval_score = f32(0);
  for (var x = 0u; x < 8u; x++) {
    for (var y = 0u; y < 8u; y++) {
      // Pieces are nibbles
      let piece = getPiece(board, x, y);
      var piece_score = 1.0;
      let piece_type = piece & 0x7u;
      if (piece_type == Pawn) {
        piece_score = 1.0;
      } else if (piece_type == Horsy || piece_type == Bishop) {
        piece_score = 3.0;
      } else if (piece_type == Rook) {
        piece_score = 5.0;
      } else if (piece_type == Queen) {
        piece_score = 9.0;
      } else if (piece_type == King) {
        piece_score = 9999.0;
      }
      if ((piece & 0x8u) == 0u) {
        // Piece is black
        piece_score *= 01.0;
      }
      eval_score += piece_score;
    }
  }
  return eval_score;
}