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

    /// Allocate memory from the stack
    pub fn alloc(&mut self, size: usize, is_data: bool) -> Option<NonNull<u8>> {
        let chunk_size = if is_data { self.data_size } else { self.meta_size };
        
        // Try to allocate from existing chunks
        for chunk in &mut self.chunks {
            if chunk.capacity - chunk.used >= size {
                let ptr = unsafe {
                    NonNull::new_unchecked(chunk.ptr.as_ptr().add(chunk.used))
                };
                chunk.used += size;
                return Some(ptr);
            }
        }

        // Create new chunk
        let alloc_size = chunk_size.max(size);
        let layout = Layout::array::<u8>(alloc_size).ok()?;
        let ptr = unsafe { alloc::alloc(layout) };
        let ptr = NonNull::new(ptr)?;

        self.chunks.push(Chunk {
            ptr,
            layout,
            used: size,
            capacity: alloc_size,
        });

        Some(ptr)
    }

    /// Allocate and copy a string
    pub fn strdup(&mut self, s: &str, is_data: bool) -> Option<NonNull<u8>> {
        let ptr = self.alloc(s.len() + 1, is_data)?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                s.as_ptr(),
                ptr.as_ptr(),
                s.len()
            );
            *ptr.as_ptr().add(s.len()) = 0;
        }
        Some(ptr)
    }
}

impl Drop for IksStack {
    fn drop(&mut self) {
        for chunk in self.chunks.drain(..) {
            unsafe {
                alloc::dealloc(chunk.ptr.as_ptr(), chunk.layout);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_alloc() {
        let mut stack = IksStack::new(128, 256);
        
        // Allocate small block
        let ptr1 = stack.alloc(64, false).unwrap();
        assert!(!ptr1.as_ptr().is_null());
        
        // Allocate string
        let s = "test string";
        let ptr2 = stack.strdup(s, true).unwrap();
        assert!(!ptr2.as_ptr().is_null());
        
        unsafe {
            let slice = std::slice::from_raw_parts(ptr2.as_ptr(), s.len());
            assert_eq!(slice, s.as_bytes());
        }
    }
} 