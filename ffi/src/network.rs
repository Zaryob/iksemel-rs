use super::parser::CParser;
use super::*;
use std::{
    io::{Read, Write},
    net::TcpStream,
    os::fd::AsRawFd,
};
pub type StreamHook = Option<unsafe extern "C" fn(*mut c_void, c_int, *mut Handle) -> c_int>;
pub type LogHook = Option<unsafe extern "C" fn(*mut c_void, *const c_char, usize, c_int)>;
pub type Notify = Option<unsafe extern "C" fn(*mut c_void, *mut AsyncEvent) -> c_int>;
#[repr(C)]
pub struct AsyncEvent {
    event: c_int,
    data0: c_int,
    data1: c_int,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CTransport {
    abi_version: c_int,
    connect:
        Option<unsafe extern "C" fn(*mut CParser, *mut *mut c_void, *const c_char, c_int) -> c_int>,
    send: Option<unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> c_int>,
    recv: Option<unsafe extern "C" fn(*mut c_void, *mut c_char, usize, c_int) -> c_int>,
    close: Option<unsafe extern "C" fn(*mut c_void)>,
    connect_async: Option<
        unsafe extern "C" fn(
            *mut CParser,
            *mut *mut c_void,
            *const c_char,
            *const c_char,
            c_int,
            *mut c_void,
            Notify,
        ) -> c_int,
    >,
}
struct Socket {
    stream: TcpStream,
    domain: String,
}
unsafe extern "C" fn connect(
    _: *mut CParser,
    out: *mut *mut c_void,
    host: *const c_char,
    port: c_int,
) -> c_int {
    let Some(host) = text(host) else {
        return 4;
    };
    let Ok(port) = u16::try_from(port) else {
        return 6;
    };
    match TcpStream::connect((host.as_str(), port)) {
        Ok(stream) => {
            *out = Box::into_raw(Box::new(Socket {
                stream,
                domain: host,
            }))
            .cast();
            0
        }
        Err(_) => 6,
    }
}
unsafe extern "C" fn send(sock: *mut c_void, data: *const c_char, len: usize) -> c_int {
    if sock.is_null() {
        return 6;
    }
    match (*(sock as *mut Socket)).stream.write_all(bytes(data, len)) {
        Ok(()) => 0,
        Err(_) => 7,
    }
}
unsafe extern "C" fn recv(
    sock: *mut c_void,
    data: *mut c_char,
    len: usize,
    timeout: c_int,
) -> c_int {
    if sock.is_null() {
        return -1;
    }
    let stream = &mut (*(sock as *mut Socket)).stream;
    let mut poll = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = libc::poll(
        &mut poll,
        1,
        if timeout < 0 {
            -1
        } else {
            timeout.saturating_mul(1000)
        },
    );
    if ready <= 0 {
        return ready;
    }
    match stream.read(std::slice::from_raw_parts_mut(data.cast(), len)) {
        Ok(0) => -1,
        Ok(n) => n as c_int,
        Err(_) => -1,
    }
}
unsafe extern "C" fn close(sock: *mut c_void) {
    if !sock.is_null() {
        drop(Box::from_raw(sock as *mut Socket));
    }
}
#[no_mangle]
pub static iks_default_transport: CTransport = CTransport {
    abi_version: 0,
    connect: Some(connect),
    send: Some(send),
    recv: Some(recv),
    close: Some(close),
    connect_async: None,
};
struct Channel {
    trans: CTransport,
    socket: *mut c_void,
}
impl Read for Channel {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = unsafe {
            self.trans
                .recv
                .ok_or_else(|| std::io::Error::other("missing recv"))?(
                self.socket,
                b.as_mut_ptr().cast(),
                b.len(),
                -1,
            )
        };
        if n < 0 {
            Err(std::io::Error::other("transport read"))
        } else {
            Ok(n as usize)
        }
    }
}
impl Write for Channel {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let result = unsafe {
            self.trans
                .send
                .ok_or_else(|| std::io::Error::other("missing send"))?(
                self.socket,
                b.as_ptr().cast(),
                b.len(),
            )
        };
        if result == 0 {
            Ok(b.len())
        } else {
            Err(std::io::Error::other("transport write"))
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub struct TlsData {
    stream: native_tls::TlsStream<Channel>,
}
#[repr(C)]
pub struct CTls {
    handshake:
        Option<unsafe extern "C" fn(*mut *mut TlsData, *const CTransport, *mut c_void) -> c_int>,
    send: Option<unsafe extern "C" fn(*mut TlsData, *const c_char, usize) -> c_int>,
    recv: Option<unsafe extern "C" fn(*mut TlsData, *mut c_char, usize, c_int) -> c_int>,
    terminate: Option<unsafe extern "C" fn(*mut TlsData)>,
}
unsafe fn handshake_domain(
    out: *mut *mut TlsData,
    trans: *const CTransport,
    sock: *mut c_void,
    domain: &str,
) -> c_int {
    if out.is_null() || trans.is_null() {
        return 9;
    }
    let Ok(connector) = native_tls::TlsConnector::new() else {
        return 9;
    };
    match connector.connect(
        domain,
        Channel {
            trans: *trans,
            socket: sock,
        },
    ) {
        Ok(stream) => {
            *out = Box::into_raw(Box::new(TlsData { stream }));
            0
        }
        Err(_) => 9,
    }
}
unsafe extern "C" fn handshake(
    out: *mut *mut TlsData,
    trans: *const CTransport,
    sock: *mut c_void,
) -> c_int {
    if trans != &iks_default_transport || sock.is_null() {
        return 8;
    }
    handshake_domain(out, trans, sock, &(*(sock as *mut Socket)).domain)
}
unsafe extern "C" fn tls_send(data: *mut TlsData, b: *const c_char, n: usize) -> c_int {
    if data.is_null() {
        return 7;
    }
    if (*data).stream.write_all(bytes(b, n)).is_ok() {
        0
    } else {
        7
    }
}
unsafe extern "C" fn tls_recv(
    data: *mut TlsData,
    b: *mut c_char,
    n: usize,
    timeout: c_int,
) -> c_int {
    if data.is_null() {
        return -1;
    }
    let channel = (*data).stream.get_mut();
    if channel.trans.connect.map(|f| f as usize) == Some(connect as *const () as usize) {
        let stream = &mut (*(channel.socket as *mut Socket)).stream;
        let _ = stream.set_read_timeout(if timeout < 0 {
            None
        } else {
            Some(std::time::Duration::from_millis(if timeout == 0 {
                1
            } else {
                timeout as u64 * 1000
            }))
        });
    }
    match (*data)
        .stream
        .read(std::slice::from_raw_parts_mut(b.cast(), n))
    {
        Ok(0) => -1,
        Ok(n) => n as c_int,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            0
        }
        Err(_) => -1,
    }
}
unsafe extern "C" fn terminate(data: *mut TlsData) {
    if !data.is_null() {
        drop(Box::from_raw(data));
    }
}
#[no_mangle]
pub static iks_default_tls: CTls = CTls {
    handshake: Some(handshake),
    send: Some(tls_send),
    recv: Some(tls_recv),
    terminate: Some(terminate),
};
pub struct Network {
    trans: CTransport,
    sock: *mut c_void,
    server: String,
    tls: *mut TlsData,
    try_tls: bool,
    digest: Option<iksemel::DigestMd5Client>,
}
impl Drop for Network {
    fn drop(&mut self) {
        unsafe {
            terminate(self.tls);
            if let Some(close) = self.trans.close {
                close(self.sock);
            }
        }
    }
}
pub unsafe fn dispatch(p: *mut CParser, event: iksemel::StreamEvent) -> c_int {
    let (kind, node) = match event {
        iksemel::StreamEvent::StreamStart(n) => (0, Some(n)),
        iksemel::StreamEvent::Stanza(n) => (1, Some(n)),
        iksemel::StreamEvent::Error(n) => (2, Some(n)),
        iksemel::StreamEvent::StreamEnd => (3, None),
    };
    if let Some(ref node) = node {
        let name = node.name().unwrap_or_default();
        if (*p).network.as_ref().is_some_and(|n| n.try_tls) {
            if name == "failure" {
                return 9;
            }
            if name == "proceed"
                && node.find_attrib("xmlns").as_deref() == Some("urn:ietf:params:xml:ns:xmpp-tls")
            {
                let net = (*p).network.as_mut().unwrap();
                let ret = handshake_domain(&mut net.tls, &net.trans, net.sock, &net.server);
                if ret != 0 {
                    return ret;
                }
                net.try_tls = false;
                let server = net.server.clone();
                super::parser::iks_parser_reset(p);
                return send_header(p, &server);
            }
        }
        if name == "challenge"
            && node.find_attrib("xmlns").as_deref() == Some("urn:ietf:params:xml:ns:xmpp-sasl")
        {
            if let Some(client) = (*p).network.as_mut().and_then(|n| n.digest.as_mut()) {
                let Ok(decoded) = iksemel::base64_decode(&node.text()) else {
                    return 2;
                };
                let Ok(decoded) = String::from_utf8(decoded) else {
                    return 2;
                };
                let response = if decoded.starts_with("rspauth=") {
                    if client.verify_rspauth(&decoded).is_err() {
                        return 7;
                    }
                    String::new()
                } else {
                    match client.process_challenge(&decoded) {
                        Ok(r) => iksemel::base64_encode(r.as_bytes()),
                        Err(_) => return 7,
                    }
                };
                return send_text(
                    p,
                    &format!(
                        "<response xmlns='urn:ietf:params:xml:ns:xmpp-sasl'>{response}</response>"
                    ),
                );
            }
        }
    }
    if let Some(hook) = (*p).stream_hook {
        let node = node.map(|n| new_tree(n)).unwrap_or(ptr::null_mut());
        hook(super::parser::iks_user_data(p), kind, node)
    } else {
        0
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_set_log_hook(p: *mut CParser, hook: LogHook) {
    if !p.is_null() {
        (*p).log_hook = hook;
    }
}
unsafe fn send_text(p: *mut CParser, data: &str) -> c_int {
    if p.is_null() {
        return 6;
    }
    let Some(net) = (*p).network.as_mut() else {
        return 6;
    };
    let ret = if !net.tls.is_null() {
        tls_send(net.tls, data.as_ptr().cast(), data.len())
    } else {
        match net.trans.send {
            Some(send) => send(net.sock, data.as_ptr().cast(), data.len()),
            None => 8,
        }
    };
    if ret == 0 {
        if let Some(log) = (*p).log_hook {
            log(
                super::parser::iks_user_data(p),
                data.as_ptr().cast(),
                data.len(),
                0,
            );
        }
    }
    ret
}
unsafe fn send_header(p: *mut CParser, to: &str) -> c_int {
    super::parser::iks_parser_reset(p);
    if let Some(n) = (*p).network.as_mut() {
        n.server = to.into();
    }
    send_text(p,&format!("<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='{}' to='{}' version='1.0'>",iksemel::escape(&(*p).namespace),iksemel::escape(to)))
}
#[no_mangle]
pub unsafe extern "C" fn iks_send_header(p: *mut CParser, to: *const c_char) -> c_int {
    if p.is_null() {
        return 6;
    }
    let Some(to) = text(to) else {
        return 6;
    };
    send_header(p, &to)
}
#[no_mangle]
pub unsafe extern "C" fn iks_send_raw(p: *mut CParser, data: *const c_char) -> c_int {
    let Some(data) = text(data) else {
        return 7;
    };
    send_text(p, &data)
}
#[no_mangle]
pub unsafe extern "C" fn iks_send(p: *mut CParser, x: *mut Handle) -> c_int {
    if x.is_null() {
        return 7;
    }
    send_text(p, &(*x).node.to_string())
}
#[no_mangle]
pub unsafe extern "C" fn iks_connect_with(
    p: *mut CParser,
    server: *const c_char,
    port: c_int,
    name: *const c_char,
    trans: *const CTransport,
) -> c_int {
    if p.is_null() || trans.is_null() {
        return 8;
    }
    if (*trans).abi_version != 0 {
        return 8;
    }
    let Some(connect) = (*trans).connect else {
        return 8;
    };
    let Some(name) = text(name) else {
        return 6;
    };
    let mut sock = ptr::null_mut();
    let ret = connect(p, &mut sock, server, port);
    if ret != 0 {
        return ret;
    }
    (*p).network = Some(Network {
        trans: *trans,
        sock,
        server: name.clone(),
        tls: ptr::null_mut(),
        try_tls: false,
        digest: None,
    });
    send_header(p, &name)
}
#[no_mangle]
pub unsafe extern "C" fn iks_connect_tcp(
    p: *mut CParser,
    server: *const c_char,
    port: c_int,
) -> c_int {
    iks_connect_with(p, server, port, server, &iks_default_transport)
}
#[no_mangle]
pub unsafe extern "C" fn iks_connect_via(
    p: *mut CParser,
    server: *const c_char,
    port: c_int,
    name: *const c_char,
) -> c_int {
    iks_connect_with(p, server, port, name, &iks_default_transport)
}
#[no_mangle]
pub unsafe extern "C" fn iks_connect_async_with(
    p: *mut CParser,
    server: *const c_char,
    port: c_int,
    name: *const c_char,
    trans: *const CTransport,
    user: *mut c_void,
    notify: Notify,
) -> c_int {
    if p.is_null() || trans.is_null() || (*trans).abi_version != 0 {
        return 8;
    }
    let Some(connect) = (*trans).connect_async else {
        return 8;
    };
    let Some(name_text) = text(name) else {
        return 6;
    };
    let mut sock = ptr::null_mut();
    let ret = connect(p, &mut sock, server, name, port, user, notify);
    if ret != 0 {
        return ret;
    }
    (*p).network = Some(Network {
        trans: *trans,
        sock,
        server: name_text,
        tls: ptr::null_mut(),
        try_tls: false,
        digest: None,
    });
    0
}
#[no_mangle]
pub unsafe extern "C" fn iks_connect_async(
    p: *mut CParser,
    server: *const c_char,
    port: c_int,
    user: *mut c_void,
    notify: Notify,
) -> c_int {
    iks_connect_async_with(
        p,
        server,
        port,
        server,
        &iks_default_transport,
        user,
        notify,
    )
}
#[no_mangle]
pub unsafe extern "C" fn iks_connect_fd(p: *mut CParser, fd: c_int) -> c_int {
    use std::os::fd::FromRawFd;
    if p.is_null() || fd < 0 {
        return 6;
    }
    let duplicate = libc::dup(fd);
    if duplicate < 0 {
        return 5;
    }
    let stream = TcpStream::from_raw_fd(duplicate);
    let sock = Box::into_raw(Box::new(Socket {
        stream,
        domain: String::new(),
    }))
    .cast();
    (*p).network = Some(Network {
        trans: iks_default_transport,
        sock,
        server: String::new(),
        tls: ptr::null_mut(),
        try_tls: false,
        digest: None,
    });
    0
}
#[no_mangle]
pub unsafe extern "C" fn iks_fd(p: *mut CParser) -> c_int {
    if p.is_null() {
        return -1;
    }
    match (*p).network.as_ref() {
        Some(net)
            if net.trans.connect.map(|f| f as usize) == Some(connect as *const () as usize) =>
        {
            (*(net.sock as *mut Socket)).stream.as_raw_fd()
        }
        _ => -1,
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_recv(p: *mut CParser, mut timeout: c_int) -> c_int {
    if p.is_null() {
        return 6;
    }
    let mut buffer = [0u8; 4096];
    loop {
        let Some(net) = (*p).network.as_mut() else {
            return 6;
        };
        let n = if !net.tls.is_null() {
            tls_recv(net.tls, buffer.as_mut_ptr().cast(), 4095, timeout)
        } else {
            match net.trans.recv {
                Some(recv) => recv(net.sock, buffer.as_mut_ptr().cast(), 4095, timeout),
                None => return 8,
            }
        };
        if n < 0 {
            return 7;
        }
        if n == 0 {
            return 0;
        }
        if n as usize > 4095 {
            return 7;
        }
        buffer[n as usize] = 0;
        if let Some(log) = (*p).log_hook {
            log(
                super::parser::iks_user_data(p),
                buffer.as_ptr().cast(),
                n as usize,
                1,
            );
        }
        let ret = super::parser::iks_parse(p, buffer.as_ptr().cast(), n as usize, 0);
        if ret != 0 {
            return ret;
        }
        timeout = 0;
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_disconnect(p: *mut CParser) {
    if !p.is_null() {
        (*p).network = None;
        super::parser::iks_parser_reset(p);
    }
}
#[no_mangle]
pub extern "C" fn iks_has_tls() -> c_int {
    1
}
#[no_mangle]
pub unsafe extern "C" fn iks_is_secure(p: *mut CParser) -> c_int {
    (!p.is_null() && (*p).network.as_ref().is_some_and(|n| !n.tls.is_null())) as c_int
}
#[no_mangle]
pub unsafe extern "C" fn iks_start_tls(p: *mut CParser) -> c_int {
    let ret = send_text(p, "<starttls xmlns='urn:ietf:params:xml:ns:xmpp-tls'/>");
    if ret == 0 {
        (*p).network.as_mut().unwrap().try_tls = true;
    }
    ret
}
#[no_mangle]
pub unsafe extern "C" fn iks_start_sasl(
    p: *mut CParser,
    kind: c_int,
    user: *const c_char,
    password: *const c_char,
) -> c_int {
    if p.is_null() {
        return 6;
    }
    let (Some(user), Some(password)) = (text(user), text(password)) else {
        return 7;
    };
    match kind {
        0 => {
            let data = format!("\0{user}\0{password}");
            send_text(
                p,
                &format!(
                    "<auth xmlns='urn:ietf:params:xml:ns:xmpp-sasl' mechanism='PLAIN'>{}</auth>",
                    iksemel::base64_encode(data.as_bytes())
                ),
            )
        }
        1 => {
            let Some(net) = (*p).network.as_mut() else {
                return 6;
            };
            let Ok(client) = iksemel::DigestMd5Client::new(&user, &password, "xmpp", &net.server)
            else {
                return 7;
            };
            net.digest = Some(client);
            send_text(
                p,
                "<auth xmlns='urn:ietf:params:xml:ns:xmpp-sasl' mechanism='DIGEST-MD5'/>",
            )
        }
        _ => 8,
    }
}
