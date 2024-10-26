//! iksemel - XML parser library in Rust
//! A port of the iksemel C library to Rust with memory safety guarantees

mod parser;
mod dom;
mod ikstack;
mod utility;

use std::fmt;
use thiserror::Error;
use std::rc::{Rc, Weak};
use std::cell::RefCell;

pub use parser::{Parser, SaxHandler};
pub use dom::DomParser;

/// XML node types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IksType {
    None,
    Tag,
    Attribute,
    CData,
}

/// XML tag types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Open,
    Close,
    Single,
}

/// Parser error types
#[derive(Error, Debug)]
pub enum IksError {
    #[error("Out of memory")]
    NoMem,
    #[error("Invalid XML")]
    BadXml,
    #[error("Hook returned error")]
    Hook,
    #[error("Network DNS error")]
    NetNoDns,
    #[error("Network socket error")]
    NetNoSock,
    #[error("Network connection error")]
    NetNoConn,
    #[error("Network read/write error")]
    NetRwErr,
    #[error("Network operation not supported")]
    NetNotSupp,
    #[error("TLS operation failed")]
    NetTlsFail,
    #[error("Network connection dropped")]
    NetDropped,
    #[error("Unknown network error")]
    NetUnknown,
    #[error("File not found")]
    FileNoFile,
    #[error("File access denied")]
    FileNoAccess,
    #[error("File read/write error")]
    FileRwErr,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// XML Node structure
#[derive(Debug)]
pub struct IksNode {
    node_type: IksType,
    name: Option<String>,
    content: Option<String>,
    attributes: Vec<(String, String)>,
    children: Vec<Rc<RefCell<IksNode>>>,
    parent: Option<Weak<RefCell<IksNode>>>,
    next: Option<Rc<RefCell<IksNode>>>,
    prev: Option<Weak<RefCell<IksNode>>>,
}

/// Result type for iksemel operations
pub type Result<T> = std::result::Result<T, IksError>;
