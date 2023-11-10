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
  let to_move = 1u << 3u;

  var out = 0u;
  for (var x = 0u; x < 8u; x++) {
    for (var y = 0u; y < 8u; y++) {
      // Pieces are nibbles
      let piece = getPiece(&board, x, y);
      if ((piece & 0x8u) == to_move) {
        if ((piece & 0x7u) == 6u) {
          // Pawn
          if (!isColour(&board, to_move, x, y+1u)) {
            var new_board = board;
            new_board.pieces[y] &= ~(0x4u << (x*4u));
            new_board.pieces[y+1u] |= ((6u+to_move) << (x*4u));
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