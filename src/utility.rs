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

/// Set custom memory allocation functions
pub fn set_mem_funcs(malloc_func: fn(usize) -> *mut u8, free_func: fn(*mut u8)) {
    unsafe {
        INIT.call_once(|| {
            ALLOCATOR.malloc_func = Some(malloc_func);
            ALLOCATOR.free_func = Some(free_func);
        });
    }
}

/// Safe string duplication
pub fn str_dup(src: Option<&str>) -> Option<String> {
    src.map(String::from)
}

/// Safe string concatenation
pub fn str_cat(dest: &mut String, src: Option<&str>) {
    if let Some(s) = src {
        dest.push_str(s);
    }
}

/// Case-insensitive string comparison
pub fn str_casecmp(a: Option<&str>, b: Option<&str>) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => {
            for (c1, c2) in a.chars().zip(b.chars()) {
                let c1 = c1.to_ascii_lowercase();
                let c2 = c2.to_ascii_lowercase();
                if c1 != c2 {
                    return c1 as i32 - c2 as i32;
                }
            }
            a.len() as i32 - b.len() as i32
        }
        _ => -1,
    }
}

/// Safe string length calculation
pub fn str_len(src: Option<&str>) -> usize {
    src.map_or(0, str::len)
}

/// XML string escaping
pub fn escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '\'' => result.push_str("&apos;"),
            '"' => result.push_str("&quot;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// XML string unescaping
pub fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            while let Some(&next) = chars.peek() {
                if next == ';' {
                    chars.next();
                    break;
                }
                entity.push(chars.next().unwrap());
            }
            
            match entity.as_str() {
                "amp" => result.push('&'),
                "apos" => result.push('\''),
                "quot" => result.push('"'),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                _ => {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_utils() {
        // Test str_dup
        assert_eq!(str_dup(Some("test")), Some("test".to_string()));
        assert_eq!(str_dup(None), None);

        // Test str_cat
        let mut s = String::from("Hello");
        str_cat(&mut s, Some(" World"));
        assert_eq!(s, "Hello World");

        // Test str_casecmp
        assert_eq!(str_casecmp(Some("test"), Some("TEST")), 0);
        assert_eq!(str_casecmp(Some("test"), Some("test2")), -1);
        assert_eq!(str_casecmp(None, Some("test")), -1);

        // Test str_len
        assert_eq!(str_len(Some("test")), 4);
        assert_eq!(str_len(None), 0);
    }

    #[test]
    fn test_xml_escaping() {
        let input = "a < b & c > d \"quote\" 'apos'";
        let escaped = escape(input);
        assert_eq!(
            escaped,
            "a &lt; b &amp; c &gt; d &quot;quote&quot; &apos;apos&apos;"
        );
        assert_eq!(unescape(&escaped), input);
    }

    #[test]
    fn test_custom_allocator() {
        static mut ALLOC_CALLED: bool = false;
        static mut FREE_CALLED: bool = false;

        unsafe {
            set_mem_funcs(
                |size| {
                    ALLOC_CALLED = true;
                    System.alloc(Layout::from_size_align_unchecked(size, 1))
                },
                |ptr| {
                    FREE_CALLED = true;
                    System.dealloc(ptr, Layout::from_size_align_unchecked(1, 1))
                }
            );

            let ptr = ALLOCATOR.malloc_func.unwrap()(10);
            assert!(ALLOC_CALLED);

            ALLOCATOR.free_func.unwrap()(ptr);
            assert!(FREE_CALLED);
        }
    }
} 