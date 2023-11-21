use std::{marker::PhantomData, rc::Rc};

use log::info;
use wgpu::{Buffer, Device, BufferUsages, BufferDescriptor};

use crate::shaders::WORKGROUP_SIZE;

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
    allocated_elements: u64,
    allocations: u64,
}

pub struct AllocToken<'mngr, T: BufferData> {
    buffer_index: usize,
    offset: u64,
    len: u64,
    mngr: &'mngr BufferManager<T>
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
        let index = self.buffers.len()+1;
        self.buffers.push(BufData {
            buffer: self.device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{} (buf #{})", self.label, index)),
                size: self.max_bytes_per_buf,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            allocated_elements: 0,
            allocations: 0,
        });
        index
    }

    /// Allocates a buffer,
    /// the size is measured in number of elements
    pub fn allocate<'mngr>(&'mngr mut self, size: u64) -> AllocToken<'mngr, T> {
        let buf_index = self.buffers.iter().enumerate().find(|(_, bufdata)| {
            let left = self.max_elements_per_buf - bufdata.allocated_elements;
            left >= size
        }).map(|(i, _)| i).unwrap_or_else(|| {
            let new_index = self.new_buffer();
            new_index
        });

        let buf = &mut self.buffers[buf_index];

        let offset = buf.allocated_elements;
        buf.allocations += 1;
        buf.allocated_elements += size;

        AllocToken {
            buffer_index: buf_index,
            offset,
            len: size,
            mngr: self
        }
    }

    pub fn dealloc<'mngr>(&'mngr mut self, token: AllocToken<'mngr, T>) {
        let buf = &mut self.buffers[token.buffer_index];
        buf.allocations -= 1;
        if buf.allocations == 0 {
            buf.allocated_elements = 0;
        }
    }
}

pub trait BufferData {
    const SIZE: usize;
}