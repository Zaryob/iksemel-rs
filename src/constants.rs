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

/// Memory allocation constants
pub mod memory {
    /// Default alignment for memory allocations (based on f64)
    pub const ALIGNMENT: usize = std::mem::align_of::<f64>();
    
    /// Bit mask for alignment calculations
    pub const ALIGN_MASK: usize = ALIGNMENT - 1;
    
    /// Minimum size for memory chunks
    pub const MIN_CHUNK_SIZE: usize = ALIGNMENT * 8;
    
    /// Minimum size for individual allocations
    pub const MIN_ALLOC_SIZE: usize = ALIGNMENT;
    
    /// Default chunk size for DOM parsing
    pub const DEFAULT_DOM_CHUNK_SIZE: usize = 4096;
    
    /// Default chunk size for IKS nodes
    pub const DEFAULT_IKS_CHUNK_SIZE: usize = 256;
    
    /// Default buffer size for file operations
    pub const FILE_BUFFER_SIZE: usize = 8192;

    /// Initial capacity for attribute vectors
    pub const INITIAL_ATTR_CAPACITY: usize = 8;

    /// Initial capacity for child node vectors
    pub const INITIAL_CHILD_CAPACITY: usize = 4;

    /// Maximum number of chunks in a stack
    pub const MAX_CHUNKS: usize = 1024;

    /// Growth factor for chunk sizes
    pub const CHUNK_GROWTH_FACTOR: f64 = 1.5;
}

/// XML parsing constants
pub mod xml {
    /// Maximum length for XML entity names
    pub const MAX_ENTITY_LENGTH: usize = 8;
    
    /// Maximum number of attributes per tag
    pub const MAX_ATTRIBUTES: usize = 32;
    
    /// Maximum length for tag names
    pub const MAX_TAG_LENGTH: usize = 256;
    
    /// Maximum length for attribute names
    pub const MAX_ATTR_NAME_LENGTH: usize = 128;
    
    /// Maximum length for attribute values
    pub const MAX_ATTR_VALUE_LENGTH: usize = 1024;

    /// Maximum nesting depth for XML elements
    pub const MAX_NESTING_DEPTH: usize = 1000;

    /// Maximum length for CDATA sections
    pub const MAX_CDATA_LENGTH: usize = 1024 * 1024; // 1MB

    /// Maximum length for XML comments
    pub const MAX_COMMENT_LENGTH: usize = 4096;
} 