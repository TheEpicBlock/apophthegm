/// A tree of chess positions that lives mainly on the gpu
#[derive(Default)]
pub struct GpuTree {
    layers: Vec<GpuTreeLayer>
}

struct GpuTreeLayer {
    num_boards: u32
}