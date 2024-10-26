use std::alloc::{self, Layout};
use std::ptr::NonNull;

/// A memory-efficient stack allocator for XML parsing
pub(crate) struct IksStack {
    chunks: Vec<Chunk>,
    meta_size: usize,
    data_size: usize,
}

struct Chunk {
    ptr: NonNull<u8>,
    layout: Layout,
    used: usize,
    capacity: usize,
}

impl IksStack {
    /// Create a new stack with given chunk sizes
    pub fn new(meta_chunk: usize, data_chunk: usize) -> Self {
        const MIN_CHUNK: usize = 128;
        let meta_size = meta_chunk.max(MIN_CHUNK);
        let data_size = data_chunk.max(MIN_CHUNK);
        
        IksStack {
            chunks: Vec::new(),
            meta_size,
            data_size,
        }
    }
}

