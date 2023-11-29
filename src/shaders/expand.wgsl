@group(0) @binding(0)
var<storage, read_write> input: array<Board>;
@group(0) @binding(1)
var<storage, read_write> output: array<Board>;
@group(0) @binding(3)
var<storage, read_write> out_index: atomic<u32>;
@group(0) @binding(2)
var<uniform> globals: GlobalData;

@compute @workgroup_size(64)
fn expansion_pass(
  @builtin(global_invocation_id)
  global_id : vec3u,

  @builtin(local_invocation_id)
  local_id : vec3u,
) {
  // Avoid accessing the buffer out of bounds
  if (global_id.x >= globals.input_size) {
    return;
  }
  var board = input[global_id.x + globals.buf_offset_0];
  let to_move = globals.to_move;

  var pawn_start_rank = 6u; // 0-indexed!
  if (to_move == 0x8u) {
    pawn_start_rank = 1u; // 0-indexed!
  }

  var pawn_promote_rank = 0u; // 0-indexed!
  if (to_move == 0x8u) {
    pawn_promote_rank = 7u; // 0-indexed!
  }


  for (var x = 0u; x < 8u; x++) {
    for (var y = 0u; y < 8u; y++) {
      // Pieces are nibbles
      let piece = getPiece(&board, x, y);
      if ((piece & 0x8u) == to_move) {
        let piece_type = piece & 0x7u;
        if (piece_type == Pawn) {
          // Pawn
          let offset = ((to_move >> 3u) * 2u) - 1u;
          // Upward movement
          if (getPiece(&board, x, y+offset) == 0u) {
            // Regular upwards move
            pawn_move(&board, x, y, x, y+offset, pawn_promote_rank, to_move, global_id.x);

            if (y == pawn_start_rank && getPiece(&board, x, y+(offset*2u)) == 0u) {
              var new_board2 = movePiece(&board, piece, x, y, x, y+(offset*2u), global_id.x);
              let out = atomicAdd(&out_index, 1u);
              output[out + globals.buf_offset_1] = new_board2;
            }
          }
          // Capture
          if (x + 1u < 8u && isOpponent(&board, to_move, x + 1u, y+offset)) {
            pawn_move(&board, x, y, x + 1u, y+offset, pawn_promote_rank, to_move, global_id.x);
          }
          // Yes, this check is still correct, because it's unsigned
          if (x - 1u < 8u && isOpponent(&board, to_move, x - 1u, y+offset)) {
            pawn_move(&board, x, y, x - 1u, y+offset, pawn_promote_rank, to_move, global_id.x);
          }
        } else if (piece_type == King) {
          try_move(&board, piece, x, y, (x - 1u), (y - 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 0u), (y - 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 1u), (y - 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x - 1u), (y + 0u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 1u), (y + 0u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x - 1u), (y + 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 0u), (y + 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 1u), (y + 1u), to_move, global_id.x);
        } else if (piece_type == Horsy) {
          try_move(&board, piece, x, y, (x + 2u), (y - 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 2u), (y + 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x - 2u), (y - 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x - 2u), (y + 1u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x - 1u), (y + 2u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 1u), (y + 2u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x - 1u), (y - 2u), to_move, global_id.x);
          try_move(&board, piece, x, y, (x + 1u), (y - 2u), to_move, global_id.x);
        } else if (piece_type == Rook || piece_type == Queen) {
          move_in_dir(&board, piece, x, y, 1, 0, to_move, global_id.x);
          move_in_dir(&board, piece, x, y, -1, 0, to_move, global_id.x);
          move_in_dir(&board, piece, x, y, 0, 1, to_move, global_id.x);
          move_in_dir(&board, piece, x, y, 0, -1, to_move, global_id.x);
        }

        if (piece_type == Bishop || piece_type == Queen) {
          move_in_dir(&board, piece, x, y, 1, 1, to_move, global_id.x);
          move_in_dir(&board, piece, x, y, 1, -1, to_move, global_id.x);
          move_in_dir(&board, piece, x, y, -1, 1, to_move, global_id.x);
          move_in_dir(&board, piece, x, y, -1, -1, to_move, global_id.x);
        }
      }
    }
  }
}

fn move_in_dir(board: ptr<function, Board>, piece: u32, x: u32, y: u32, dx: i32, dy: i32, to_move: u32, prev: u32) {
  var xNew = x + u32(dx);
  var yNew = y + u32(dy);
  if (xNew >= 8u) { return; }
  if (yNew >= 8u) { return; }
  if (isColour(board, to_move, xNew, yNew)) { return; }
  var new_board = *board;
  new_board.pieces[y] &= ~(0xFu << (x*4u)); // Remove the original piece
  loop {
    let target_square = getPiece(board, xNew, yNew);
    if (target_square != 0u && (target_square & 0x8u) == to_move) {
      // Trying to move to a square with an own piece
      return;
    }
    new_board.pieces[yNew] &= ~(0xFu << (xNew*4u));
    new_board.pieces[yNew] |= (piece << (xNew*4u));
    setPrev(&new_board, prev);
    let out = atomicAdd(&out_index, 1u);
    output[out + globals.buf_offset_1] = new_board;

    if (target_square != 0u && (target_square & 0x8u) != to_move) {
      // This was a capture, no more moves
      return;
    }

    xNew = xNew + u32(dx);
    yNew = yNew + u32(dy);
    if (xNew >= 8u) { return; }
    if (yNew >= 8u) { return; }
    new_board.pieces[yNew - u32(dy)] &= ~(0xFu << ((xNew - u32(dx))*4u));
  }
}

fn try_move(board: ptr<function, Board>, piece: u32, x: u32, y: u32, xNew: u32, yNew: u32, to_move: u32, prev: u32) {
  if (xNew >= 8u) { return; }
  if (yNew >= 8u) { return; }
  if (!isColour(board, to_move, xNew, yNew)) {
    var new_board = movePiece(board, piece, x, y, xNew, yNew, prev);
    let out = atomicAdd(&out_index, 1u);
    output[out + globals.buf_offset_1] = new_board;
  }
}

fn pawn_move(board: ptr<function, Board>, x: u32, y: u32, xNew: u32, yNew: u32, pawn_promote_rank: u32, to_move: u32, prev: u32) {
  if (yNew == pawn_promote_rank) {
    // Promote
    var new_board = *board;
    new_board.pieces[y] &= ~(0xFu << (x*4u)); // Remove the original pawn
    setPrev(&new_board, prev);
    let clear_mask = ~(0xFu << (xNew*4u));
    let out = atomicAdd(&out_index, 4u);
    new_board.pieces[yNew] &= clear_mask;
    new_board.pieces[yNew] |= ((Queen | to_move) << (xNew*4u));
    output[out + globals.buf_offset_1] = new_board;
    new_board.pieces[yNew] &= clear_mask;
    new_board.pieces[yNew] |= ((Bishop | to_move) << (xNew*4u));
    output[out+1u + globals.buf_offset_1] = new_board;
    new_board.pieces[yNew] &= clear_mask;
    new_board.pieces[yNew] |= ((Horsy | to_move) << (xNew*4u));
    output[out+2u + globals.buf_offset_1] = new_board;
    new_board.pieces[yNew] &= clear_mask;
    new_board.pieces[yNew] |= ((Rook | to_move) << (xNew*4u));
    output[out+3u + globals.buf_offset_1] = new_board;
  } else {
    var new_board = movePiece(board, (Pawn | to_move), x, y, xNew, yNew, prev);
    let out = atomicAdd(&out_index, 1u);
    output[out + globals.buf_offset_1] = new_board;
  }
}

fn isColour(board: ptr<function, Board>, colour: u32, x: u32, y: u32) -> bool {
  let p = getPiece(board, x, y);
  return p != 0u && ((p & 0x8u) == colour);
}

fn isOpponent(board: ptr<function, Board>, to_move: u32, x: u32, y: u32) -> bool {
  let p = getPiece(board, x, y);
  return p != 0u && ((p & 0x8u) != to_move);
}

fn movePiece(board: ptr<function, Board>, piece: u32, x: u32, y: u32, xNew: u32, yNew: u32, prev: u32) -> Board {
  var new_board = *board;
  new_board.pieces[y] &= ~(0xFu << (x*4u));
  new_board.pieces[yNew] &= ~(0xFu << (xNew*4u));
  new_board.pieces[yNew] |= (piece << (xNew*4u));
  setPrev(&new_board, prev);
  return new_board;
}

fn setPrev(board: ptr<function, Board>, id: u32) {
  (*board).pieces[8] = id;
}