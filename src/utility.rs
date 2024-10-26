use std::sync::Once;
use std::alloc::{GlobalAlloc, System, Layout};

/// Custom memory allocator wrapper
struct IksAllocator {
    malloc_func: Option<fn(usize) -> *mut u8>,
    free_func: Option<fn(*mut u8)>,
}

static mut ALLOCATOR: IksAllocator = IksAllocator {
    malloc_func: None,
    free_func: None,
};

static INIT: Once = Once::new();
