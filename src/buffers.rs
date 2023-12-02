use std::{marker::PhantomData, rc::Rc, sync::atomic::AtomicI32, ops::{RangeBounds, Range, Index, Deref}, cell::{RefCell, Ref}, borrow::BorrowMut};

use log::{info, debug};
use wgpu::{Buffer, Device, BufferUsages, BufferDescriptor, BufferAddress, MapMode, BufferAsyncError, BufferView, CommandEncoderDescriptor, Queue, MAP_ALIGNMENT, BufferBinding};

use crate::{shaders::WORKGROUP_SIZE, misc::{SliceExtension, self}};

pub struct BufferManager<ElementType: BufferData> {
    label: &'static str,
    buffers: RefCell<Vec<BufData>>,
    staging: Buffer,
    max_elements_per_buf: u32,
    max_bricks_per_buf: u32,
    max_bytes_per_buf: u64,
    device: Rc<Device>,
    a: PhantomData<ElementType>,
}

struct BufData {
    buffer: Buffer,
    allocated_bricks: u32,
    allocations: u64,
    times_mapped: AtomicI32,
}

pub struct AllocToken<T: BufferData> {
    buffer_index: usize,
    /// Measured in bytes
    offset: BufferAddress,
    // Measured in bricks
    len: u32,
    // Measured in elements
    len_elems: u32,
    // Measured in bytes
    len_bytes: BufferAddress,
    a: PhantomData<T>
}

impl<T: BufferData> BufferManager<T> {
    const BRICK_SIZE: u32 = misc::lcm(T::SIZE as u32, MAP_ALIGNMENT as u32);
    const ELEMS_PER_BRICK: u32 = Self::BRICK_SIZE / T::SIZE as u32;

    pub fn create(device: Rc<Device>, max_elems_per_buf: u32, label: &'static str) -> Self {
        let max_bricks_per_buf = max_elems_per_buf / Self::ELEMS_PER_BRICK;
        let buffer_size = max_bricks_per_buf as u64 * Self::BRICK_SIZE as u64;
        debug!("({label})");
        Self {
            label,
            buffers: RefCell::new(Vec::new()),
            staging: device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{} (staging buf)", label)),
                size: buffer_size,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            max_elements_per_buf: max_elems_per_buf,
            max_bytes_per_buf: buffer_size,
            max_bricks_per_buf,
            device,
            a: PhantomData::default(),
        }
    }

    fn new_buffer(&self) -> usize {
        info!("({}) Allocating new buffer", self.label);
        let mut buffers = self.buffers.borrow_mut();
        let index = buffers.len();
        buffers.push(BufData {
            buffer: self.device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{} (buf #{})", self.label, index)),
                size: self.max_bytes_per_buf,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            allocated_bricks: 0,
            allocations: 0,
            times_mapped: AtomicI32::new(0),
        });
        index
    }

    /// Allocates a buffer,
    /// the size is measured in number of elements
    pub fn allocate(&self, size: u32) -> AllocToken<T> {
        let bricks_needed = misc::ceil_div(size, Self::ELEMS_PER_BRICK as u64);
        assert!(bricks_needed <= self.max_bricks_per_buf, "{bricks_needed} > {}", self.max_bricks_per_buf);

        let buffers = self.buffers.borrow();
        let buf_index = buffers.iter().enumerate().find(|(_, bufdata)| {
            let bricks_left = self.max_bricks_per_buf - bufdata.allocated_bricks;
            bricks_left >= bricks_needed
        }).map(|(i, _)| i);
        drop(buffers);
        
        let buf_index = buf_index.unwrap_or_else(|| {
            let new_index = self.new_buffer();
            new_index
        });

        let mut buffers = self.buffers.borrow_mut();
        let buf = &mut buffers[buf_index];

        buf.allocations += 1;
        let offset = buf.allocated_bricks as u64 * Self::BRICK_SIZE as u64;
        buf.allocated_bricks += bricks_needed;

        debug!("({}) Allocated {size} elements ({} bricks, {} bytes) to buffer #{buf_index} at {offset}", self.label, bricks_needed, bricks_needed as u64 * Self::BRICK_SIZE as u64);

        AllocToken {
            buffer_index: buf_index,
            offset,
            len: bricks_needed,
            len_elems: bricks_needed * Self::ELEMS_PER_BRICK,
            len_bytes: bricks_needed as u64 * Self::BRICK_SIZE as u64,
            a: PhantomData::default(),
        }
    }

    pub fn dealloc<'mngr>(&self, token: AllocToken<T>) {
        debug!("({}) Freeing {} elements ({} bricks, {} bytes) from buffer #{} at {}", self.label, token.len(), token.len, token.byte_len(), token.buffer_index, token.offset);
        let buf = &mut self.buffers.borrow_mut()[token.buffer_index];
        buf.allocations -= 1;
        if buf.allocations == 0 {
            buf.allocated_bricks = 0;
        }
    }

    pub async fn view(&self, queue: &Queue, token: &AllocToken<T>, bounds: Range<u32>) -> Result<BufView<'_, T>, BufferAsyncError> {
        assert!((0..token.len()).contains(&bounds.start), "{:?} is not contained in {:?}", bounds, 0..token.len());
        assert!((0..token.len()+1).contains(&bounds.end), "{:?} is not contained in {:?}", bounds, 0..token.len());
        let bound_len = bounds.end-bounds.start;

        if bound_len == 0 {
            return Ok(BufView::Empty);
        }

        let mut command_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());
        command_encoder.copy_buffer_to_buffer(
            &token.buffer(self),
            token.start(), // Source offset
            &self.staging,
            token.start(), // Destination offset
            token.byte_len(),
        );
        queue.submit([command_encoder.finish()]);

        let buf = &self.buffers.borrow()[token.buffer_index];
        let buf_slice = self.staging.slice((token.start()+(bounds.start as u64*T::SIZE as u64))..(token.start()+bound_len as u64*T::SIZE as u64));
        buf.times_mapped.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        buf_slice.map_buffer(&self.device, MapMode::Read).await?;
        let view = buf_slice.get_mapped_range();
        Ok(BufView::Normal(BufViewData {
            wgpu_view: Some(view),
            buffer: &self.staging,
            counter: self.buffers.borrow(),
            index: token.buffer_index,
            a: PhantomData::default(),
        }))
    }
}

pub struct IndexRef<'a, B> {
    inner: Ref<'a, Vec<B>>,
    index: usize
}

impl<T> Deref for IndexRef<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.inner[self.index]
    }
}

pub struct BufDeref<'a> {
    inner: IndexRef<'a, BufData>,
}

impl Deref for BufDeref<'_> {
    type Target = Buffer;

    #[inline]
    fn deref(&self) -> &Buffer {
        &self.inner.buffer
    }
}

impl<T> AllocToken<T> where T: BufferData {
    pub fn buffer<'a>(&self, mngr: &'a BufferManager<T>) -> impl Deref<Target = Buffer> + 'a  {
        BufDeref {
            inner: self.buf_data(mngr),
        }
    }

    fn buf_data<'a>(&self, mngr: &'a BufferManager<T>) -> IndexRef<'a, BufData> {
        IndexRef {
            inner: mngr.buffers.borrow(),
            index: self.buffer_index,
        }
    }

    /// The first address in the buffer of this allocation slice
    pub fn start(&self) -> BufferAddress {
        self.offset
    }

    /// The last address (exclusive) in the buffer of this allocation slice
    pub fn end(&self) -> BufferAddress {
        self.offset + self.byte_len()
    }

    /// The first element in the buffer of this allocation slice
    pub fn start_elem(&self) -> u32 {
        (self.offset / T::SIZE as u64) as u32
    }

    /// The amount of elements that this allocation slice can store
    pub fn len(&self) -> u32 {
        self.len_elems
    }

    /// The length in bytes
    pub fn byte_len(&self) -> u64 {
        self.len_bytes
    }
}

pub trait BufferData {
    const SIZE: usize;
}

pub enum BufView<'a, T: BufferData> {
    Empty,
    Normal(BufViewData<'a, T>)
}

pub struct BufViewData<'a, T: BufferData> {
    wgpu_view: Option<BufferView<'a>>,
    buffer: &'a Buffer,
    counter: Ref<'a, Vec<BufData>>,
    index: usize,
    a: PhantomData<T>,
}

impl<'a, T: BufferData> Drop for BufView<'a, T> {
    fn drop(&mut self) {
        if let Self::Normal(data) = self {
            let view = data.wgpu_view.take();
            drop(view);
            let prev = data.counter[data.index].times_mapped.fetch_add(-1, std::sync::atomic::Ordering::SeqCst);
            if prev == 1 {
                data.buffer.unmap();
            }
        }
    }
}

impl<'a, T: BufferData + bytemuck::Pod> AsRef<[T]> for BufView<'a, T> {
    fn as_ref(&self) -> &[T] {
        match self {
            Self::Normal(data) => {
                let view = data.wgpu_view.as_ref().unwrap();
                return bytemuck::cast_slice(view.as_ref());
            }
            Self::Empty => {
                return &[];
            }
        }
    }
}

impl<'a, T: BufferData> AsRef<[u8]> for BufView<'a, T> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Normal(data) => {
                let view = data.wgpu_view.as_ref().unwrap();
                return view.as_ref();
            }
            Self::Empty => {
                return &[];
            }
        }
    }
}

impl<'a, T: BufferData + bytemuck::Pod> BufView<'a, T> {
    pub fn cast_t(&self) -> &[T] {
        self.as_ref()
    }
}

#[cfg(test)]
mod test {
    use crate::{buffers::BufferManager, chess::{test::GPU_ADAPTER, EvalScore}, gpu::init_gpu_evaluator};

    #[tokio::test]
    async fn test() {
        let engine = init_gpu_evaluator(&GPU_ADAPTER).await;
        let mngr = BufferManager::<EvalScore>::create(engine.device, 512, "test buf");
        let alloc_a = mngr.allocate(1);
        let alloc_b = mngr.allocate(1);
        println!("Alloc A {} .. {}", alloc_a.start(), alloc_a.end());
        println!("Alloc B {} .. {}", alloc_b.start(), alloc_b.end());
        assert!(alloc_a.start() < alloc_a.end());
        assert!(alloc_b.start() < alloc_b.end());
        assert!(alloc_a.end() <= alloc_b.start() || alloc_b.end() <= alloc_a.start());
        mngr.dealloc(alloc_b);
        let alloc_b = mngr.allocate(1);
        println!("Alloc B2 {} .. {}", alloc_b.start(), alloc_b.end());
        assert!(alloc_a.end() <= alloc_b.start() || alloc_b.end() <= alloc_a.start());
        
        // Not part of the contract for the manager, but a sanity check to see if buffers are being cleared
        let prev_a_start = alloc_a.start();
        let prev_a_end = alloc_a.end();
        mngr.dealloc(alloc_a);
        mngr.dealloc(alloc_b);
        let alloc_a = mngr.allocate(1);
        println!("Alloc A2 {} .. {}", alloc_a.start(), alloc_a.end());
        assert_eq!(alloc_a.start(), prev_a_start);
        assert_eq!(alloc_a.end(), prev_a_end);
    }

    #[tokio::test]
    async fn test_big() {
        let engine = init_gpu_evaluator(&GPU_ADAPTER).await;
        let mngr = BufferManager::<EvalScore>::create(engine.device, 512, "test buf");
        let alloc_a = mngr.allocate(300);
        let alloc_b = mngr.allocate(300);
        // Both of these should be in a new buffer (they don't fit into a single one)
        assert_eq!(alloc_a.start(), 0);
        assert_eq!(alloc_b.start(), 0);
    }
}