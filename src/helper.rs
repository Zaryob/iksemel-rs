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

use crate::constants::memory;

/// Aligns a size to the default alignment.
/// 
/// This function ensures that the given size is aligned to the default
/// alignment boundary, which is based on the size of an f64.
/// 
/// # Arguments
/// 
/// * `size` - The size to align
/// 
/// # Returns
/// 
/// The aligned size
pub fn align_size(size: usize) -> usize {
    ((size + memory::ALIGNMENT - 1) & !memory::ALIGN_MASK)
}

/// Calculates chunk growth size based on current size and growth factor.
/// 
/// This function is used to determine the new size when a chunk needs to grow.
/// It applies the growth factor and ensures the result is properly aligned.
/// 
/// # Arguments
/// 
/// * `current_size` - The current size of the chunk
/// 
/// # Returns
/// 
/// The new size for the chunk
pub fn calculate_chunk_growth(current_size: usize) -> usize {
    let growth = (current_size as f64 * memory::CHUNK_GROWTH_FACTOR) as usize;
    align_size(growth)
}

/// Calculates the size needed for escaping a string.
/// 
/// This function determines how many characters will be needed to escape
/// special XML characters in the input string.
/// 
/// # Arguments
/// 
/// * `s` - The string to calculate escape size for
/// 
/// # Returns
/// 
/// The number of characters needed to escape the string
pub fn escape_size(s: &str) -> usize {
    s.chars().map(|c| match c {
        '&' => 5,  // &amp;
        '<' => 4,  // &lt;
        '>' => 4,  // &gt;
        '"' => 6,  // &quot;
        '\'' => 6, // &apos;
        _ => 1,
    }).sum()
}

/// Calculates the size needed for unescaping a string.
/// 
/// This function determines how many characters will be needed to unescape
/// XML entities in the input string.
/// 
/// # Arguments
/// 
/// * `s` - The string to calculate unescape size for
/// 
/// # Returns
/// 
/// The number of characters needed to unescape the string
pub fn unescape_size(s: &str) -> usize {
    let mut size = 0;
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
                "amp" | "lt" | "gt" => size += 1,
                "quot" | "apos" => size += 1,
                _ => size += entity.len() + 2, // &entity;
            }
        } else {
            size += 1;
        }
    }
    size
} 