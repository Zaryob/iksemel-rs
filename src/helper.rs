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

/// Align a size to the default alignment
pub fn align_size(size: usize) -> usize {
    ((size + memory::ALIGNMENT - 1) & !memory::ALIGN_MASK)
}

/// Calculate chunk growth size based on current size and growth factor
pub fn calculate_chunk_growth(current_size: usize) -> usize {
    let growth = (current_size as f64 * memory::CHUNK_GROWTH_FACTOR) as usize;
    align_size(growth)
}

/// Calculate the size needed for escaping a string
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

/// Calculate the size needed for unescaping a string
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