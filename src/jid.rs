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

use std::fmt;
use std::str::FromStr;
use crate::{IksError, Result};

/// Represents a Jabber Identifier (JID) according to RFC 6122 / RFC 7622.
///
/// A JID is composed of three parts:
/// - `node` (localpart, optional): e.g. "user" in "user@example.com/home"
/// - `domain` (domainpart, required): e.g. "example.com"
/// - `resource` (resourcepart, optional): e.g. "home"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Jid {
    node: Option<String>,
    domain: String,
    resource: Option<String>,
}

impl Jid {
    /// Parses a string into a validated JID.
    ///
    /// # Arguments
    ///
    /// * `s` - The JID string to parse (e.g., "user@domain/resource")
    ///
    /// # Returns
    ///
    /// A parsed and normalized `Jid` or an error if invalid.
    pub fn new(s: &str) -> Result<Self> {
        if s.is_empty() || s.len() > 3071 {
            return Err(IksError::BadJid);
        }

        // Split into [node@]domain[/resource]
        let (node, rest) = if let Some(at_idx) = s.find('@') {
            let n = &s[..at_idx];
            let r = &s[at_idx + 1..];
            (Some(n), r)
        } else {
            (None, s)
        };

        let (domain, resource) = if let Some(slash_idx) = rest.find('/') {
            let d = &rest[..slash_idx];
            let res = &rest[slash_idx + 1..];
            (d, Some(res))
        } else {
            (rest, None)
        };

        Self::from_parts(node, domain, resource)
    }

    /// Creates a JID from separate parts and validates them.
    pub fn from_parts(node: Option<&str>, domain: &str, resource: Option<&str>) -> Result<Self> {
        let valid_node = match node {
            Some(n) => {
                if n.is_empty() || n.len() > 1023 || !is_valid_node(n) {
                    return Err(IksError::BadJid);
                }
                Some(n.to_ascii_lowercase())
            }
            None => None,
        };

        if domain.is_empty() || domain.len() > 1023 || !is_valid_domain(domain) {
            return Err(IksError::BadJid);
        }
        let valid_domain = domain.to_ascii_lowercase();

        let valid_resource = match resource {
            Some(r) => {
                if r.is_empty() || r.len() > 1023 {
                    return Err(IksError::BadJid);
                }
                Some(r.to_string())
            }
            None => None,
        };

        Ok(Jid {
            node: valid_node,
            domain: valid_domain,
            resource: valid_resource,
        })
    }

    /// Gets the node (localpart) of the JID, if present.
    pub fn node(&self) -> Option<&str> {
        self.node.as_deref()
    }

    /// Gets the domain (domainpart) of the JID.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Gets the resource (resourcepart) of the JID, if present.
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }

    /// Returns the bare JID string representation ("node@domain" or "domain").
    pub fn bare(&self) -> String {
        match &self.node {
            Some(n) => format!("{}@{}", n, self.domain),
            None => self.domain.clone(),
        }
    }

    /// Returns a new JID with only the bare components (without resource).
    pub fn as_bare(&self) -> Self {
        Jid {
            node: self.node.clone(),
            domain: self.domain.clone(),
            resource: None,
        }
    }

    /// Returns the full JID string representation ("node@domain/resource" or "domain/resource").
    pub fn full(&self) -> String {
        self.to_string()
    }

    /// Returns whether this JID has no resource part.
    pub fn is_bare(&self) -> bool {
        self.resource.is_none()
    }

    /// Returns whether this JID has a resource part.
    pub fn is_full(&self) -> bool {
        self.resource.is_some()
    }

    /// Returns whether this JID consists only of a domain (no node and no resource).
    pub fn is_domain_only(&self) -> bool {
        self.node.is_none() && self.resource.is_none()
    }

    /// Returns a new JID with the specified resource attached.
    pub fn with_resource(&self, res: &str) -> Result<Self> {
        Self::from_parts(self.node(), self.domain(), Some(res))
    }

    /// Returns a new JID without any resource part.
    pub fn without_resource(&self) -> Self {
        self.as_bare()
    }

    /// Checks if this JID matches another JID according to iksemel matching semantics.
    /// If either JID has no resource, compares their bare parts.
    /// If both have resources, compares full JIDs.
    pub fn matches(&self, other: &Jid) -> bool {
        if self.domain != other.domain || self.node != other.node {
            return false;
        }
        match (&self.resource, &other.resource) {
            (Some(r1), Some(r2)) => r1 == r2,
            _ => true,
        }
    }
}

/// Checks if a localpart (node) adheres to RFC 6122 rules.
fn is_valid_node(node: &str) -> bool {
    for c in node.chars() {
        match c {
            '"' | '&' | '\'' | '/' | ':' | '<' | '>' | '@' => return false,
            _ if c.is_control() || c.is_whitespace() => return false,
            _ => {}
        }
    }
    true
}

/// Checks if a domainpart adheres to basic hostname / IP rules.
fn is_valid_domain(domain: &str) -> bool {
    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    for c in domain.chars() {
        match c {
            '@' | '/' | ':' | '<' | '>' => return false,
            _ if c.is_control() || c.is_whitespace() => return false,
            _ => {}
        }
    }
    true
}

impl fmt::Display for Jid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref node) = self.node {
            write!(f, "{}@", node)?;
        }
        write!(f, "{}", self.domain)?;
        if let Some(ref res) = self.resource {
            write!(f, "/{}", res)?;
        }
        Ok(())
    }
}

impl FromStr for Jid {
    type Err = IksError;

    fn from_str(s: &str) -> Result<Self> {
        Jid::new(s)
    }
}

impl PartialEq<&str> for Jid {
    fn eq(&self, other: &&str) -> bool {
        match Jid::new(other) {
            Ok(parsed) => self == &parsed,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_jids() {
        let jid = Jid::new("user@example.com/home").unwrap();
        assert_eq!(jid.node(), Some("user"));
        assert_eq!(jid.domain(), "example.com");
        assert_eq!(jid.resource(), Some("home"));

        let jid_bare = Jid::new("alice@jabber.org").unwrap();
        assert_eq!(jid_bare.node(), Some("alice"));
        assert_eq!(jid_bare.domain(), "jabber.org");
        assert_eq!(jid_bare.resource(), None);

        let jid_domain = Jid::new("conference.example.org").unwrap();
        assert_eq!(jid_domain.node(), None);
        assert_eq!(jid_domain.domain(), "conference.example.org");
        assert_eq!(jid_domain.resource(), None);
    }

    #[test]
    fn test_invalid_jids() {
        assert!(Jid::new("").is_err());
        assert!(Jid::new("@domain.com").is_err());
        assert!(Jid::new("user@").is_err());
        assert!(Jid::new("user@/resource").is_err());
        assert!(Jid::new("user with spaces@domain.com").is_err());
        assert!(Jid::new("user<bad>@domain.com").is_err());
    }

    #[test]
    fn test_bare_and_full() {
        let jid = Jid::new("Bob@Example.COM/Mobile").unwrap();
        // Case normalization on node and domain
        assert_eq!(jid.node(), Some("bob"));
        assert_eq!(jid.domain(), "example.com");
        assert_eq!(jid.resource(), Some("Mobile"));
        assert_eq!(jid.bare(), "bob@example.com");
        assert_eq!(jid.full(), "bob@example.com/Mobile");
        assert!(jid.is_full());
        assert!(!jid.is_bare());

        let bare_jid = jid.without_resource();
        assert!(bare_jid.is_bare());
        assert_eq!(bare_jid.to_string(), "bob@example.com");

        let with_res = bare_jid.with_resource("Desktop").unwrap();
        assert_eq!(with_res.resource(), Some("Desktop"));
        assert_eq!(with_res.full(), "bob@example.com/Desktop");
    }

    #[test]
    fn test_jid_matches() {
        let full = Jid::new("user@example.com/phone").unwrap();
        let bare = Jid::new("user@example.com").unwrap();
        let diff_res = Jid::new("user@example.com/laptop").unwrap();
        let other_user = Jid::new("other@example.com").unwrap();

        assert!(full.matches(&bare));
        assert!(bare.matches(&full));
        assert!(!full.matches(&diff_res));
        assert!(!full.matches(&other_user));
    }

    #[test]
    fn test_jid_string_equality() {
        let jid = Jid::new("user@example.com").unwrap();
        assert_eq!(jid, "user@example.com");
        assert_ne!(jid, "other@example.com");
    }
}
