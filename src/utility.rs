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



/// Safely duplicates a string.
/// 
/// This function provides a safe way to duplicate a string, handling
/// the case where the input is None.
/// 
/// # Arguments
/// 
/// * `src` - Optional string to duplicate
/// 
/// # Returns
/// 
/// An `Option` containing the duplicated string
pub fn str_dup(src: Option<&str>) -> Option<String> {
    src.map(String::from)
}

/// Safely concatenates strings.
/// 
/// This function provides a safe way to concatenate strings, handling
/// the case where the source is None.
/// 
/// # Arguments
/// 
/// * `dest` - The destination string to append to
/// * `src` - Optional string to append
pub fn str_cat(dest: &mut String, src: Option<&str>) {
    if let Some(s) = src {
        dest.push_str(s);
    }
}

/// Performs case-insensitive string comparison.
/// 
/// This function compares two strings ignoring case, handling the case
/// where either string is None.
/// 
/// # Arguments
/// 
/// * `a` - First string to compare
/// * `b` - Second string to compare
/// 
/// # Returns
/// 
/// A negative number if `a` is less than `b`, 0 if they are equal,
/// or a positive number if `a` is greater than `b`
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
        (None, None) => 0,
        (Some(_), None) => 1,
        (None, Some(_)) => -1,
    }
}

/// Safely calculates string length.
/// 
/// This function provides a safe way to get the length of a string,
/// handling the case where the input is None.
/// 
/// # Arguments
/// 
/// * `src` - Optional string to get length of
/// 
/// # Returns
/// 
/// The length of the string, or 0 if the input is None
pub fn str_len(src: Option<&str>) -> usize {
    src.map_or(0, str::len)
}

/// Escapes special XML characters in a string.
/// 
/// This function replaces special XML characters with their corresponding
/// XML entities.
/// 
/// # Arguments
/// 
/// * `s` - The string to escape
/// 
/// # Returns
/// 
/// The escaped string
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

/// Unescapes XML entities in a string.
/// 
/// This function replaces XML entities with their corresponding characters.
/// 
/// # Arguments
/// 
/// * `s` - The string to unescape
/// 
/// # Returns
/// 
/// The unescaped string
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
                _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                    if let Ok(code) = u32::from_str_radix(&entity[2..], 16) {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                            continue;
                        }
                    }
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                }
                _ if entity.starts_with('#') => {
                    if let Ok(code) = entity[1..].parse::<u32>() {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                            continue;
                        }
                    }
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                }
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
        assert_eq!(str_dup(Some("test")), Some("test".to_string()));
        assert_eq!(str_dup(None), None);

        let mut s = String::from("Hello");
        str_cat(&mut s, Some(" World"));
        assert_eq!(s, "Hello World");

        assert_eq!(str_casecmp(Some("test"), Some("TEST")), 0);
        assert_eq!(str_casecmp(Some("test"), Some("test2")), -1);
        assert_eq!(str_casecmp(None, Some("test")), -1);

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
} 