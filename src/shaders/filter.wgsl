@group(0) @binding(0)
var<uniform> globals: GlobalData;
@group(0) @binding(1)
var<storage, read_write> input: array<Board>;
@group(0) @binding(2)
var<storage, read_write> output: array<Board>;
@group(0) @binding(3)
var<storage, read_write> evals: array<u32>;
@group(0) @binding(4)
var<storage, read_write> out_index: atomic<u32>;

@compute @workgroup_size(64)
fn filter_pass(
  @builtin(global_invocation_id)
  global_id : vec3u,

  @builtin(local_invocation_id)
  local_id : vec3u,
) {
  // Avoid accessing the buffer out of bounds
  if (global_id.x >= globals.input_size) {
    return;
  }
  var board = input[global_id.x + globals.buf_offset_1];
  var eval = evals[global_id.x + globals.buf_offset_3];
  if eval == globals.buf_offset_0 { // Yeah, I'm storing the eval in the buf offset instead of making a new variable for it, what are you going to do about it
    let out = atomicAdd(&out_index, 1u);
    output[out + globals.buf_offset_2] = board;
  }
}