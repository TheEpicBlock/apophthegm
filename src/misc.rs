use wgpu::{BufferSlice, MapMode, BufferAsyncError, Device};

pub trait SliceExtension {
    async fn map_buffer(&self, device: &Device, mode: MapMode) -> Result<(), BufferAsyncError>;
}

impl SliceExtension for BufferSlice<'_> {
    async fn map_buffer(&self, device: &Device, mode: MapMode) -> Result<(), BufferAsyncError> {
        let (tx, rx) = futures_channel::oneshot::channel::<Result<(), BufferAsyncError>>();
        self.map_async(mode, |result| {
            tx.send(result).expect("Receiver should never be dropped");
        });
        while !device.poll(wgpu::MaintainBase::Poll) {
            tokio::task::yield_now().await;
        }
        device.poll(wgpu::MaintainBase::Wait);

        return rx.await.expect("Sender should never be dropper");
    }
}

pub fn ceil_div(a: u32, b: u64) -> u32 {
    return (a as f64 / b as f64).ceil() as u32;
}

pub const fn lcm(a: u32, b: u32) -> u32 {
    (a*b) / gcd(a, b)
}

pub const fn gcd(a: u32, b: u32) -> u32 {
    let mut a = a;
    let mut b = b;
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    };
    return a;
}