@group(0) @binding(0)
var<uniform> globals: GlobalData;
@group(0) @binding(1)
var<storage, read> child_boards: array<Board>;
@group(0) @binding(2)
var<storage, read_write> child_evals: array<u32>;
@group(0) @binding(3)
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
  var board = child_boards[global_id.x + globals.buf_offset_1];
  let prev_index = getPrev(&board, globals.move_index);

  switch globals.to_move {
    case 0x8u: {
      atomicMax(&parent_evals[prev_index + globals.buf_offset_3], child_evals[global_id.x + globals.buf_offset_2]);
    }
    case 0x0u: {
      atomicMin(&parent_evals[prev_index + globals.buf_offset_3], child_evals[global_id.x + globals.buf_offset_2]);
    }
    default: {}
  }
}