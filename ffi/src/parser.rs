use super::*;
use iksemel::{DomParser, IksError, Parser, SaxHandler, TagType};
pub type TagHook =
    Option<unsafe extern "C" fn(*mut c_void, *mut c_char, *mut *mut c_char, c_int) -> c_int>;
pub type DataHook = Option<unsafe extern "C" fn(*mut c_void, *mut c_char, usize) -> c_int>;
pub type DeleteHook = Option<unsafe extern "C" fn(*mut c_void)>;
#[derive(Clone, Copy)]
struct Hooks {
    user: *mut c_void,
    tag: TagHook,
    data: DataHook,
}
impl SaxHandler for Hooks {
    fn on_tag(
        &mut self,
        name: &str,
        attributes: &[(String, String)],
        kind: TagType,
    ) -> iksemel::Result<()> {
        if let Some(hook) = self.tag {
            let name = std::ffi::CString::new(name).map_err(|_| IksError::BadXml)?;
            let fields: Vec<_> = attributes
                .iter()
                .flat_map(|(key, value)| [key, value])
                .map(|v| std::ffi::CString::new(v.as_str()))
                .collect::<std::result::Result<_, _>>()
                .map_err(|_| IksError::BadXml)?;
            let mut pointers: Vec<_> = fields.iter().map(|s| s.as_ptr() as *mut c_char).collect();
            pointers.push(ptr::null_mut());
            let attrs = if attributes.is_empty() {
                ptr::null_mut()
            } else {
                pointers.as_mut_ptr()
            };
            let result = unsafe {
                hook(
                    self.user,
                    name.as_ptr() as *mut c_char,
                    attrs,
                    kind as c_int,
                )
            };
            if result != 0 {
                return Err(IksError::Hook);
            }
        }
        Ok(())
    }
    fn on_cdata(&mut self, data: &str) -> iksemel::Result<()> {
        if let Some(hook) = self.data {
            let mut data = data.as_bytes().to_vec();
            data.push(0);
            if unsafe { hook(self.user, data.as_mut_ptr().cast(), data.len() - 1) } != 0 {
                return Err(IksError::Hook);
            }
        }
        Ok(())
    }
}
enum Mode {
    Sax(Parser<Hooks>),
    Dom(Parser<DomParser>, *mut *mut Handle),
    Stream(iksemel::StreamParser),
}
pub struct CParser {
    mode: Mode,
    hooks: Hooks,
    delete: DeleteHook,
    pub(crate) stack: *mut Stack,
    carry: Vec<u8>,
    nr_bytes: usize,
    nr_lines: usize,
    pub(crate) stream_hook: network::StreamHook,
    pub(crate) log_hook: network::LogHook,
    pub(crate) namespace: String,
    pub(crate) network: Option<network::Network>,
}
fn error(error: IksError) -> c_int {
    match error {
        IksError::NoMem => 1,
        IksError::Hook => 3,
        _ => 2,
    }
}
unsafe fn make(mode: Mode, hooks: Hooks, stack: *mut Stack, delete: DeleteHook) -> *mut CParser {
    unsafe {
        Box::into_raw(Box::new(CParser {
            mode,
            hooks,
            delete,
            stack: if stack.is_null() {
                iks_stack_new(0, 0)
            } else {
                stack
            },
            carry: Vec::new(),
            nr_bytes: 0,
            nr_lines: 0,
            stream_hook: None,
            log_hook: None,
            namespace: "jabber:client".into(),
            network: None,
        }))
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_sax_new(
    user: *mut c_void,
    tag: TagHook,
    data: DataHook,
) -> *mut CParser {
    unsafe {
        let hooks = Hooks { user, tag, data };
        make(Mode::Sax(Parser::new(hooks)), hooks, ptr::null_mut(), None)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_sax_extend(
    stack: *mut Stack,
    user: *mut c_void,
    tag: TagHook,
    data: DataHook,
    delete: DeleteHook,
) -> *mut CParser {
    unsafe {
        let hooks = Hooks { user, tag, data };
        make(Mode::Sax(Parser::new(hooks)), hooks, stack, delete)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_dom_new(out: *mut *mut Handle) -> *mut CParser {
    unsafe {
        if out.is_null() {
            return ptr::null_mut();
        }
        *out = ptr::null_mut();
        let hooks = Hooks {
            user: out.cast(),
            tag: None,
            data: None,
        };
        make(
            Mode::Dom(Parser::new(DomParser::new().unwrap()), out),
            hooks,
            ptr::null_mut(),
            None,
        )
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stream_new(
    namespace: *const c_char,
    user: *mut c_void,
    hook: network::StreamHook,
) -> *mut CParser {
    unsafe {
        let hooks = Hooks {
            user,
            tag: None,
            data: None,
        };
        let parser = make(
            Mode::Stream(iksemel::StreamParser::new()),
            hooks,
            ptr::null_mut(),
            None,
        );
        (*parser).namespace = text(namespace).unwrap_or_else(|| "jabber:client".into());
        (*parser).stream_hook = hook;
        parser
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_parser_stack(p: *mut CParser) -> *mut Stack {
    unsafe {
        if p.is_null() {
            ptr::null_mut()
        } else {
            (*p).stack
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_user_data(p: *mut CParser) -> *mut c_void {
    unsafe {
        if p.is_null() {
            ptr::null_mut()
        } else {
            (*p).hooks.user
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stream_user_data(p: *mut CParser) -> *mut c_void {
    unsafe { iks_user_data(p) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_nr_bytes(p: *mut CParser) -> libc::c_ulong {
    unsafe { if p.is_null() { 0 } else { (*p).nr_bytes as _ } }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_nr_lines(p: *mut CParser) -> libc::c_ulong {
    unsafe { if p.is_null() { 0 } else { (*p).nr_lines as _ } }
}
unsafe fn publish(p: *mut CParser) -> c_int {
    unsafe {
        if let Mode::Dom(parser, out) = &mut (*p).mode
            && let Some(node) = parser.handler().document()
            && ((**out).is_null() || !Rc::ptr_eq(&(***out).node.0, &node.0))
        {
            **out = new_tree(node);
        }
        0
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_parse(
    p: *mut CParser,
    input: *const c_char,
    len: usize,
    _finish: c_int,
) -> c_int {
    unsafe {
        if p.is_null() {
            return 2;
        }
        for &byte in bytes(input, len) {
            (*p).carry.push(byte);
            let chunk = match std::str::from_utf8(&(*p).carry) {
                Ok(chunk) => chunk.to_string(),
                Err(error) if error.error_len().is_none() => continue,
                Err(_) => return 2,
            };
            // C counts the successfully examined bytes before the bad byte.
            let mut events = Vec::new();
            let result = match &mut (*p).mode {
                Mode::Sax(parser) => parser.parse(&chunk),
                Mode::Dom(parser, _) => parser.parse(&chunk),
                Mode::Stream(parser) => parser.parse_chunk(&chunk).map(|parsed| events = parsed),
            };
            if let Err(failure) = result {
                return error(failure);
            }
            for event in events {
                let result = network::dispatch(p, event);
                if result != 0 {
                    return result;
                }
            }
            (*p).nr_bytes += (*p).carry.len();
            (*p).nr_lines += chunk.bytes().filter(|b| *b == b'\n').count();
            (*p).carry.clear();
            publish(p);
        }
        let result = match &mut (*p).mode {
            Mode::Sax(parser) => parser.flush_text(),
            Mode::Dom(parser, _) => parser.flush_text(),
            Mode::Stream(_) => Ok(()),
        };
        match result {
            Ok(()) => 0,
            Err(e) => error(e),
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_parser_reset(p: *mut CParser) {
    unsafe {
        if p.is_null() {
            return;
        }
        (*p).carry.clear();
        (*p).nr_bytes = 0;
        (*p).nr_lines = 0;
        match &mut (*p).mode {
            Mode::Sax(parser) => *parser = Parser::new((*p).hooks),
            Mode::Dom(parser, out) => {
                *parser = Parser::new(DomParser::new().unwrap());
                **out = ptr::null_mut();
            }
            Mode::Stream(parser) => parser.reset(),
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_parser_delete(p: *mut CParser) {
    unsafe {
        if !p.is_null() {
            let p = Box::from_raw(p);
            if let Some(delete) = p.delete {
                delete(p.hooks.user);
            }
            iks_stack_delete(p.stack);
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_set_size_hint(_: *mut CParser, _: usize) {}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_tree(
    input: *const c_char,
    len: usize,
    err: *mut c_int,
) -> *mut Handle {
    unsafe {
        let mut node = ptr::null_mut();
        let p = iks_dom_new(&mut node);
        let result = iks_parse(p, input, len, 1);
        if !err.is_null() {
            *err = result;
        }
        iks_parser_delete(p);
        node
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_load(filename: *const c_char, out: *mut *mut Handle) -> c_int {
    unsafe {
        if out.is_null() {
            return 6;
        }
        *out = ptr::null_mut();
        let Some(filename) = text(filename) else {
            return 4;
        };
        match std::fs::read(filename) {
            Ok(data) => {
                let mut error = 0;
                *out = iks_tree(data.as_ptr().cast(), data.len(), &mut error);
                error
            }
            Err(error) => match error.kind() {
                std::io::ErrorKind::NotFound => 4,
                std::io::ErrorKind::PermissionDenied => 5,
                _ => 6,
            },
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_save(filename: *const c_char, x: *mut Handle) -> c_int {
    unsafe {
        let Some(filename) = text(filename) else {
            return 4;
        };
        if x.is_null() {
            return 6;
        }
        match std::fs::write(filename, (*x).node.to_string()) {
            Ok(()) => 0,
            Err(error) => match error.kind() {
                std::io::ErrorKind::NotFound => 4,
                std::io::ErrorKind::PermissionDenied => 5,
                _ => 6,
            },
        }
    }
}
