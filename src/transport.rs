//! Pluggable transport/TLS and byte-accurate traffic observation.
use crate::{IksError, Result};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite};

pub trait Transport: Read + Write + Send {}
impl<T: Read + Write + Send> Transport for T {}
pub trait AsyncTransport: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> AsyncTransport for T {}

pub trait TlsBackend {
    fn upgrade(
        &self,
        stream: Box<dyn Transport>,
        domain: &str,
        verify: bool,
    ) -> Result<Box<dyn Transport>>;
}
pub struct NativeTlsBackend;
impl TlsBackend for NativeTlsBackend {
    fn upgrade(
        &self,
        stream: Box<dyn Transport>,
        domain: &str,
        verify: bool,
    ) -> Result<Box<dyn Transport>> {
        let mut builder = native_tls::TlsConnector::builder();
        builder
            .danger_accept_invalid_certs(!verify)
            .danger_accept_invalid_hostnames(!verify);
        let connector = builder.build().map_err(|_| IksError::NetTlsFail)?;
        Ok(Box::new(
            connector
                .connect(domain, stream)
                .map_err(|_| IksError::NetTlsFail)?,
        ))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ByteCounts {
    pub incoming: u64,
    pub outgoing: u64,
}
pub type LogHook = Arc<dyn Fn(Direction, &[u8]) + Send + Sync>;
#[derive(Clone, Default)]
pub(crate) struct Traffic(Arc<Mutex<(ByteCounts, Option<LogHook>)>>);
impl Traffic {
    pub fn hook(&self, hook: Option<LogHook>) {
        self.0.lock().unwrap().1 = hook;
    }
    pub fn counts(&self) -> ByteCounts {
        self.0.lock().unwrap().0
    }
    pub fn record(&self, direction: Direction, data: &[u8]) {
        let hook = {
            let mut state = self.0.lock().unwrap();
            match direction {
                Direction::Incoming => state.0.incoming += data.len() as u64,
                Direction::Outgoing => state.0.outgoing += data.len() as u64,
            }
            state.1.clone()
        };
        if let Some(hook) = hook {
            hook(direction, data);
        }
    }
}
