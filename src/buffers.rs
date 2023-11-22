use std::{marker::PhantomData, rc::Rc, sync::atomic::AtomicI32, ops::{RangeBounds, Range}};

use log::info;
use wgpu::{Buffer, Device, BufferUsages, BufferDescriptor, BufferAddress, MapMode, BufferAsyncError, BufferView, CommandEncoderDescriptor, Queue};

use crate::{shaders::WORKGROUP_SIZE, misc::SliceExtension};

pub struct BufferManager<ElementType: BufferData> {
    label: &'static str,
    buffers: Vec<BufData>,
    staging: Buffer,
    max_elements_per_buf: u64,
    max_bytes_per_buf: u64,
    device: Rc<Device>,
    a: PhantomData<ElementType>,
}

struct BufData {
    buffer: Buffer,
    allocated_bytes: BufferAddress,
    allocations: u64,
    times_mapped: AtomicI32,
}

pub struct AllocToken<T: BufferData> {
    buffer_index: usize,
    offset: BufferAddress,
    len: u64,
    bytes_len: u64,
    a: PhantomData<T>
}

impl<T: BufferData> BufferManager<T> {
    pub fn create(device: Rc<Device>, max_elems_per_buf: u64, label: &'static str) -> Self {
        let buffer_size = max_elems_per_buf * T::SIZE as u64;
        Self {
            label,
            buffers: Vec::new(),
            staging: device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{} (staging buf)", label)),
                size: buffer_size,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            max_elements_per_buf: max_elems_per_buf,
            max_bytes_per_buf: buffer_size,
            device,
            a: PhantomData::default(),
        }
    }

    fn new_buffer(&mut self) -> usize {
        let index = self.buffers.len();
        self.buffers.push(BufData {
            buffer: self.device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{} (buf #{})", self.label, index)),
                size: self.max_bytes_per_buf,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            allocated_bytes: 0,
            allocations: 0,
            times_mapped: AtomicI32::new(0),
        });
        index
    }

    /// Allocates a buffer,
    /// the size is measured in number of elements
    pub fn allocate(&mut self, size: u64) -> AllocToken<T> {
        let bytes_size = size * T::SIZE as u64;

        let buf_index = self.buffers.iter().enumerate().find(|(_, bufdata)| {
            let bytes_left = self.max_bytes_per_buf - bufdata.allocated_bytes;
            bytes_left >= bytes_size;
            false
        }).map(|(i, _)| i).unwrap_or_else(|| {
            let new_index = self.new_buffer();
            new_index
        });

        let buf = &mut self.buffers[buf_index];

        buf.allocations += 1;
        let offset = buf.allocated_bytes;
        let mut end = buf.allocated_bytes + size;
        // Align end
        let alignment = self.device.limits().min_storage_buffer_offset_alignment as u64;
        end = end / alignment + if end % alignment == 0 { 0 } else { alignment }; 
        buf.allocated_bytes = end;

        AllocToken {
            buffer_index: buf_index,
            offset,
            len: size,
            bytes_len: bytes_size,
            a: PhantomData::default(),
        }
    }

    pub fn dealloc<'mngr>(&mut self, token: AllocToken<T>) {
        let buf = &mut self.buffers[token.buffer_index];
        buf.allocations -= 1;
        if buf.allocations == 0 {
            buf.allocated_bytes = 0;
        }
    }

    pub async fn view(&self, queue: &Queue, token: &AllocToken<T>, bounds: Range<BufferAddress>) -> Result<BufView<'_, T>, BufferAsyncError> {
        assert!((0..token.len()).contains(&bounds.start));
        assert!((0..token.len()).contains(&bounds.end));
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

        let buf = &self.buffers[token.buffer_index];
        let buf_slice = self.staging.slice((token.start()+bounds.start)..(token.start()+bound_len*T::SIZE as u64));
        buf.times_mapped.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        buf_slice.map_buffer(&self.device, MapMode::Read).await?;
        let view = buf_slice.get_mapped_range();
        Ok(BufView::Normal {
            wgpu_view: Some(view),
            buffer: &self.staging,
            counter: &buf.times_mapped,
            a: PhantomData::default(),
        })
    }
}

impl<T> AllocToken<T> where T: BufferData {
    pub fn buffer<'a>(&self, mngr: &'a BufferManager<T>) -> &'a Buffer {
        return &mngr.buffers[self.buffer_index].buffer;
    }

    /// The first address in the buffer of this allocation slice
    pub fn start(&self) -> BufferAddress {
        self.offset
    }

    /// The last address in the buffer of this allocation slice
    pub fn end(&self) -> BufferAddress {
        self.offset + self.byte_len()
    }

    /// The amount of elements that this allocation slice can store
    pub fn len(&self) -> u64 {
        self.len
    }

    /// The length in bytes
    pub fn byte_len(&self) -> u64 {
        self.bytes_len
    }
}

pub trait BufferData {
    const SIZE: usize;
}

pub enum BufView<'a, T: BufferData> {
    Empty,
    Normal {
        wgpu_view: Option<BufferView<'a>>,
        buffer: &'a Buffer,
        counter: &'a AtomicI32,
        a: PhantomData<T>,
    }
}

impl<'a, T: BufferData> Drop for BufView<'a, T> {
    fn drop(&mut self) {
        if let Self::Normal { wgpu_view, buffer, counter, a: _ } = self {
            let view = wgpu_view.take();
            drop(view);
            let prev = counter.fetch_add(-1, std::sync::atomic::Ordering::SeqCst);
            if prev == 1 {
                buffer.unmap();
            }
        }
    }
}

impl<'a, T: BufferData + bytemuck::Pod> AsRef<[T]> for BufView<'a, T> {
    fn as_ref(&self) -> &[T] {
        match self {
            Self::Normal { wgpu_view, buffer: _, counter: _, a: _ } => {
                let view = wgpu_view.as_ref().unwrap();
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
            Self::Normal { wgpu_view, buffer: _, counter: _, a: _ } => {
                let view = wgpu_view.as_ref().unwrap();
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