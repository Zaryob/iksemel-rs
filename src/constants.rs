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

/// Memory allocation constants.
/// 
/// This module contains constants related to memory allocation and management
/// in the XML parser. These constants are used to optimize memory usage and
/// ensure proper alignment of allocated memory.
pub mod memory {
    /// Default alignment for memory allocations (based on f64).
    /// 
    /// This constant defines the default alignment for memory allocations.
    /// It is based on the size of an f64 to ensure optimal performance
    /// on most architectures.
    pub const ALIGNMENT: usize = std::mem::align_of::<f64>();
    
    /// Bit mask for alignment calculations.
    /// 
    /// This constant is used to efficiently calculate aligned sizes.
    /// It is derived from ALIGNMENT and is used in bitwise operations.
    pub const ALIGN_MASK: usize = ALIGNMENT - 1;
    
    /// Minimum size for memory chunks.
    /// 
    /// This constant defines the minimum size for memory chunks used
    /// in the stack allocator. It ensures that chunks are large enough
    /// to be efficient while not wasting too much memory.
    pub const MIN_CHUNK_SIZE: usize = ALIGNMENT * 8;
    
    /// Minimum size for individual allocations.
    /// 
    /// This constant defines the minimum size for individual memory
    /// allocations. It ensures that even small allocations are properly
    /// aligned and efficient.
    pub const MIN_ALLOC_SIZE: usize = ALIGNMENT;
    
    /// Default chunk size for DOM parsing.
    /// 
    /// This constant defines the default size for memory chunks used
    /// during DOM parsing. It is optimized for typical XML document sizes.
    pub const DEFAULT_DOM_CHUNK_SIZE: usize = 4096;
    
    /// Default chunk size for IKS nodes.
    /// 
    /// This constant defines the default size for memory chunks used
    /// for IKS node allocations. It is optimized for typical node sizes.
    pub const DEFAULT_IKS_CHUNK_SIZE: usize = 256;
    
    /// Default buffer size for file operations.
    /// 
    /// This constant defines the default size for buffers used in
    /// file operations. It is optimized for typical file I/O performance.
    pub const FILE_BUFFER_SIZE: usize = 8192;

    /// Initial capacity for attribute vectors.
    /// 
    /// This constant defines the initial capacity for vectors that
    /// store XML attributes. It is chosen to minimize reallocations
    /// for typical XML documents.
    pub const INITIAL_ATTR_CAPACITY: usize = 8;

    /// Initial capacity for child node vectors.
    /// 
    /// This constant defines the initial capacity for vectors that
    /// store child nodes. It is chosen to minimize reallocations
    /// for typical XML documents.
    pub const INITIAL_CHILD_CAPACITY: usize = 4;

    /// Maximum number of chunks in a stack.
    /// 
    /// This constant defines the maximum number of memory chunks that
    /// can be allocated in the stack allocator. It prevents unbounded
    /// memory growth.
    pub const MAX_CHUNKS: usize = 1024;

    /// Growth factor for chunk sizes.
    /// 
    /// This constant defines the factor by which chunk sizes grow when
    /// more memory is needed. It is chosen to balance memory usage and
    /// allocation frequency.
    pub const CHUNK_GROWTH_FACTOR: f64 = 1.5;
}

/// XML parsing constants.
/// 
/// This module contains constants related to XML parsing and validation.
/// These constants define limits and constraints for XML processing.
pub mod xml {
    /// Maximum length for XML entity names.
    /// 
    /// This constant defines the maximum length allowed for XML entity
    /// names. It helps prevent buffer overflows and excessive memory usage.
    pub const MAX_ENTITY_LENGTH: usize = 8;
    
    /// Maximum number of attributes per tag.
    /// 
    /// This constant defines the maximum number of attributes allowed
    /// on a single XML tag. It helps prevent excessive memory usage
    /// and potential DoS attacks.
    pub const MAX_ATTRIBUTES: usize = 32;
    
    /// Maximum length for tag names.
    /// 
    /// This constant defines the maximum length allowed for XML tag
    /// names. It helps prevent buffer overflows and excessive memory usage.
    pub const MAX_TAG_LENGTH: usize = 256;
    
    /// Maximum length for attribute names.
    /// 
    /// This constant defines the maximum length allowed for XML attribute
    /// names. It helps prevent buffer overflows and excessive memory usage.
    pub const MAX_ATTR_NAME_LENGTH: usize = 128;
    
    /// Maximum length for attribute values.
    /// 
    /// This constant defines the maximum length allowed for XML attribute
    /// values. It helps prevent buffer overflows and excessive memory usage.
    pub const MAX_ATTR_VALUE_LENGTH: usize = 1024;

    /// Maximum nesting depth for XML elements.
    /// 
    /// This constant defines the maximum nesting depth allowed for XML
    /// elements. It helps prevent stack overflow and excessive memory usage.
    pub const MAX_NESTING_DEPTH: usize = 1000;

    /// Maximum length for CDATA sections.
    /// 
    /// This constant defines the maximum length allowed for XML CDATA
    /// sections. It helps prevent excessive memory usage and potential
    /// DoS attacks.
    pub const MAX_CDATA_LENGTH: usize = 1024 * 1024; // 1MB

    /// Maximum length for XML comments.
    /// 
    /// This constant defines the maximum length allowed for XML comments.
    /// It helps prevent excessive memory usage and potential DoS attacks.
    pub const MAX_COMMENT_LENGTH: usize = 4096;
} 