/* 
            iksemel - XML parser for Rust
          Copyright (C) 2024 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the Affero General Public License
 as published by the Free Software Foundation; either version 3
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 Affero General Public License for more details.
*/

use std::alloc::{self, Layout};
use std::ptr::NonNull;
use crate::constants::memory;
use crate::helper::{align_size, calculate_chunk_growth};

/// A memory-efficient stack allocator for XML parsing.
/// 
/// This structure provides a custom memory allocator that uses a stack-based
/// approach for efficient memory management during XML parsing. It allocates
/// memory in chunks and manages them as a stack, which is particularly well-suited
/// for XML parsing where memory allocations and deallocations follow a LIFO pattern.
/// 
/// # Examples
/// 
/// ```
/// use iksemel::ikstack::IksStack;
/// 
/// let mut stack = IksStack::new(1024, 2048);
/// 
/// // Allocate memory
/// let ptr = stack.alloc(64, false).unwrap();
/// 
/// // Allocate and copy a string
/// let s = "test string";
/// let ptr2 = stack.strdup(s, true).unwrap();
/// 
/// // Concatenate strings
/// let ptr3 = stack.strcat(Some(ptr2), " more text").unwrap();
/// ```
pub struct IksStack {
    chunks: Vec<Chunk>,
    meta_size: usize,
    data_size: usize,
    allocated: usize,
}

/// Represents a chunk of allocated memory in the stack.
struct Chunk {
    ptr: NonNull<u8>,
    layout: Layout,
    used: usize,
    capacity: usize,
    last: usize,
}

impl IksStack {
    /// Creates a new stack with given chunk sizes.
    /// 
    /// # Arguments
    /// 
    /// * `meta_chunk` - Size of chunks for metadata allocations
    /// * `data_chunk` - Size of chunks for data allocations
    /// 
    /// # Returns
    /// 
    /// A new `IksStack` instance
    pub fn new(meta_chunk: usize, data_chunk: usize) -> Self {
        let meta_size = align_size(meta_chunk.max(memory::MIN_CHUNK_SIZE));
        let data_size = align_size(data_chunk.max(memory::MIN_CHUNK_SIZE));
        
        IksStack {
            chunks: Vec::new(),
            meta_size,
            data_size,
            allocated: 0,
        }
    }

    /// Allocates memory from the stack.
    /// 
    /// This method attempts to allocate memory from existing chunks first,
    /// and creates a new chunk if necessary.
    /// 
    /// # Arguments
    /// 
    /// * `size` - The size of memory to allocate
    /// * `is_data` - Whether this is a data allocation (affects chunk size)
    /// 
    /// # Returns
    /// 
    /// An `Option` containing a pointer to the allocated memory
    pub fn alloc(&mut self, size: usize, is_data: bool) -> Option<NonNull<u8>> {
        let size = size.max(memory::MIN_ALLOC_SIZE);
        let size = align_size(size);
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

        self.allocated += alloc_size;
        self.chunks.push(Chunk {
            ptr,
            layout,
            used: size,
            capacity: alloc_size,
            last: 0,
        });

        Some(ptr)
    }

    /// Allocates and copies a string.
    /// 
    /// This method allocates memory and copies the input string into it,
    /// ensuring proper null termination.
    /// 
    /// # Arguments
    /// 
    /// * `s` - The string to duplicate
    /// * `is_data` - Whether this is a data allocation
    /// 
    /// # Returns
    /// 
    /// An `Option` containing a pointer to the duplicated string
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

    /// Concatenates strings efficiently.
    /// 
    /// This method concatenates an existing string with a new string,
    /// allocating new memory as needed.
    /// 
    /// # Arguments
    /// 
    /// * `old` - Optional pointer to existing string
    /// * `src` - The string to append
    /// 
    /// # Returns
    /// 
    /// An `Option` containing a pointer to the concatenated string
    pub fn strcat(&mut self, old: Option<NonNull<u8>>, src: &str) -> Option<NonNull<u8>> {
        if old.is_none() {
            return self.strdup(src, true);
        }

        let old_len = unsafe { strlen(old.unwrap().as_ptr()) };
        let src_len = src.len();
        let total_len = old_len + src_len;

        let ptr = self.alloc(total_len + 1, true)?;
        unsafe {
            if let Some(old_ptr) = old {
                std::ptr::copy_nonoverlapping(
                    old_ptr.as_ptr(),
                    ptr.as_ptr(),
                    old_len
                );
            }
            std::ptr::copy_nonoverlapping(
                src.as_ptr(),
                ptr.as_ptr().add(old_len),
                src_len
            );
            *ptr.as_ptr().add(total_len) = 0;
        }
        Some(ptr)
    }

    /// Gets statistics about memory usage.
    /// 
    /// # Returns
    /// 
    /// A tuple containing (total_allocated, total_used)
    pub fn stats(&self) -> (usize, usize) {
        let mut used = 0;
        for chunk in &self.chunks {
            used += chunk.used;
        }
        (self.allocated, used)
    }
}

impl Drop for IksStack {
    /// Frees all allocated memory when the stack is dropped.
    fn drop(&mut self) {
        for chunk in self.chunks.drain(..) {
            unsafe {
                alloc::dealloc(chunk.ptr.as_ptr(), chunk.layout);
            }
        }
    }
}

/// Calculates the length of a null-terminated string.
/// 
/// # Arguments
/// 
/// * `ptr` - Pointer to the start of the string
/// 
/// # Returns
/// 
/// The length of the string
unsafe fn strlen(ptr: *const u8) -> usize {
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    len
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

    #[test]
    fn test_strcat() {
        let mut stack = IksStack::new(128, 256);
        
        let s1 = "Hello";
        let s2 = " World";
        let ptr1 = stack.strdup(s1, true).unwrap();
        let ptr2 = stack.strcat(Some(ptr1), s2).unwrap();
        
        unsafe {
            let slice = std::slice::from_raw_parts(ptr2.as_ptr(), s1.len() + s2.len());
            assert_eq!(slice, (s1.to_string() + s2).as_bytes());
        }
    }
} 