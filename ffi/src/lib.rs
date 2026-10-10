//! C ABI adapter. All unsafe pointer handling stays outside the safe core.
//!
//! # Safety contract
//! Callers must supply valid, NUL-terminated UTF-8 strings (or explicit byte lengths),
//! writable output buffers of the documented size, and live handles obtained from this
//! library. Handles are confined to the creating thread. Tree and stack pointers become
//! invalid together on deletion. Callbacks must not free the invoking parser/filter.
#![allow(clippy::missing_safety_doc)]
use iksemel::{IksType, NodeRef, PacketJid};
use libc::{c_char, c_int, c_void};
use std::{collections::HashMap, ffi::CStr, ptr, rc::Rc};
mod network;
mod packet;
mod parser;

#[derive(Default)]
pub struct Stack {
    blocks: Vec<*mut c_void>,
    // Boxed so raw pointers handed to C stay valid when the Vec reallocates.
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<Handle>>,
    node_index: HashMap<(usize, Option<String>), *mut Handle>,
}
impl Drop for Stack {
    fn drop(&mut self) {
        for block in self.blocks.drain(..) {
            unsafe {
                libc::free(block);
            }
        }
    }
}
pub struct Handle {
    node: NodeRef,
    stack: *mut Stack,
    attribute: Option<String>,
}
unsafe fn bytes<'a>(s: *const c_char, len: usize) -> &'a [u8] {
    unsafe {
        if s.is_null() {
            return &[];
        }
        if len == 0 {
            CStr::from_ptr(s).to_bytes()
        } else {
            std::slice::from_raw_parts(s.cast(), len)
        }
    }
}
unsafe fn text(s: *const c_char) -> Option<String> {
    unsafe {
        if s.is_null() {
            None
        } else {
            CStr::from_ptr(s).to_str().ok().map(str::to_string)
        }
    }
}
unsafe fn data(s: *const c_char, len: usize) -> Option<String> {
    unsafe { std::str::from_utf8(bytes(s, len)).ok().map(str::to_string) }
}
// C's serializer skips malformed UTF-8 byte sequences. Keep valid fragments
// for explicitly sized CDATA, while XML parsing rejects malformed UTF-8.
fn valid_text(mut input: &[u8]) -> String {
    let mut out = String::new();
    while !input.is_empty() {
        match std::str::from_utf8(input) {
            Ok(valid) => {
                out.push_str(valid);
                break;
            }
            Err(e) => {
                out.push_str(std::str::from_utf8(&input[..e.valid_up_to()]).unwrap());
                input = &input
                    [e.valid_up_to() + e.error_len().unwrap_or(input.len() - e.valid_up_to())..];
            }
        }
    }
    out
}
unsafe fn alloc(s: *mut Stack, len: usize) -> *mut c_void {
    unsafe {
        let block = libc::calloc(len.max(1), 1);
        if !block.is_null() && !s.is_null() {
            (*s).blocks.push(block);
        }
        block
    }
}
unsafe fn string(s: *mut Stack, value: &str) -> *mut c_char {
    unsafe {
        let out = alloc(s, value.len() + 1).cast::<c_char>();
        if !out.is_null() {
            ptr::copy_nonoverlapping(value.as_ptr(), out.cast(), value.len());
        }
        out
    }
}
unsafe fn handle(s: *mut Stack, node: NodeRef, attribute: Option<String>) -> *mut Handle {
    unsafe {
        let key = (Rc::as_ptr(&node.0) as usize, attribute.clone());
        if let Some(handle) = (*s).node_index.get(&key) {
            return *handle;
        }
        let mut value = Box::new(Handle {
            node,
            stack: s,
            attribute,
        });
        let out = &mut *value as *mut Handle;
        (*s).nodes.push(value);
        (*s).node_index.insert(key, out);
        out
    }
}
unsafe fn new_tree(node: NodeRef) -> *mut Handle {
    unsafe {
        let stack = Box::into_raw(Box::<Stack>::default());
        handle(stack, node, None)
    }
}
unsafe fn lookup(x: *mut Handle, node: Option<NodeRef>) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        node.map(|node| handle((*x).stack, node, None))
            .unwrap_or(ptr::null_mut())
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_malloc(size: usize) -> *mut c_void {
    unsafe { libc::malloc(size.max(1)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_free(p: *mut c_void) {
    unsafe { libc::free(p) }
}
/// Custom global allocators cannot replace Rust allocation; documented no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_set_mem_funcs(
    _: Option<unsafe extern "C" fn(usize) -> *mut c_void>,
    _: Option<unsafe extern "C" fn(*mut c_void)>,
) {
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack_new(_: usize, _: usize) -> *mut Stack {
    Box::into_raw(Box::default())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack_delete(s: *mut Stack) {
    unsafe {
        if !s.is_null() {
            drop(Box::from_raw(s));
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack_alloc(s: *mut Stack, len: usize) -> *mut c_void {
    unsafe { alloc(s, len) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack_strdup(
    s: *mut Stack,
    src: *const c_char,
    len: usize,
) -> *mut c_char {
    unsafe {
        if src.is_null() {
            return ptr::null_mut();
        }
        let input = bytes(src, len);
        let p = alloc(s, input.len() + 1).cast();
        ptr::copy_nonoverlapping(input.as_ptr(), p, input.len());
        p.cast()
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack_strcat(
    s: *mut Stack,
    old: *mut c_char,
    old_len: usize,
    src: *const c_char,
    len: usize,
) -> *mut c_char {
    unsafe {
        let mut data = bytes(old, old_len).to_vec();
        data.extend(bytes(src, len));
        let out = alloc(s, data.len() + 1).cast();
        ptr::copy_nonoverlapping(data.as_ptr(), out, data.len());
        out.cast()
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack_stat(_: *mut Stack, allocated: *mut usize, used: *mut usize) {
    unsafe {
        if !allocated.is_null() {
            *allocated = 0;
        }
        if !used.is_null() {
            *used = 0;
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strdup(src: *const c_char) -> *mut c_char {
    unsafe { iks_stack_strdup(ptr::null_mut(), src, 0) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strcat(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    unsafe {
        if dest.is_null() {
            return iks_strdup(src);
        }
        if src.is_null() {
            return dest;
        }
        let end = CStr::from_ptr(dest).to_bytes().len();
        let input = CStr::from_ptr(src).to_bytes_with_nul();
        ptr::copy(input.as_ptr(), dest.add(end).cast(), input.len());
        dest
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strlen(s: *const c_char) -> usize {
    unsafe { bytes(s, 0).len() }
}
unsafe fn cmp(a: *const c_char, b: *const c_char, n: Option<usize>, fold: bool) -> c_int {
    unsafe {
        if a.is_null() || b.is_null() {
            return if a == b {
                0
            } else if a.is_null() {
                -1
            } else {
                1
            };
        }
        let a = CStr::from_ptr(a).to_bytes();
        let b = CStr::from_ptr(b).to_bytes();
        let n = n.unwrap_or(a.len().max(b.len()) + 1);
        for i in 0..n {
            let mut x = *a.get(i).unwrap_or(&0);
            let mut y = *b.get(i).unwrap_or(&0);
            if fold {
                x = x.to_ascii_lowercase();
                y = y.to_ascii_lowercase();
            }
            if x != y {
                return x as c_int - y as c_int;
            }
            if x == 0 {
                return 0;
            }
        }
        0
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strcmp(a: *const c_char, b: *const c_char) -> c_int {
    unsafe { cmp(a, b, None, false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strcasecmp(a: *const c_char, b: *const c_char) -> c_int {
    unsafe { cmp(a, b, None, true) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strncmp(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    unsafe { cmp(a, b, Some(n), false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_strncasecmp(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    unsafe { cmp(a, b, Some(n), true) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_escape(s: *mut Stack, src: *const c_char, len: usize) -> *mut c_char {
    unsafe {
        data(src, len)
            .map(|v| string(s, &iksemel::escape(&v)))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_unescape(
    s: *mut Stack,
    src: *const c_char,
    len: usize,
) -> *mut c_char {
    unsafe {
        data(src, len)
            .map(|v| string(s, &iksemel::unescape(&v)))
            .unwrap_or(ptr::null_mut())
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_new(name: *const c_char) -> *mut Handle {
    unsafe {
        text(name)
            .map(|name| new_tree(NodeRef::new_tag(name)))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_new_within(name: *const c_char, s: *mut Stack) -> *mut Handle {
    unsafe {
        if s.is_null() {
            return iks_new(name);
        }
        text(name)
            .map(|name| handle(s, NodeRef::new_tag(name), None))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_insert(x: *mut Handle, name: *const c_char) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        text(name)
            .map(|name| {
                handle(
                    (*x).stack,
                    (*x).node.add_child(NodeRef::new_tag(name)),
                    None,
                )
            })
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_insert_node(x: *mut Handle, y: *mut Handle) -> *mut Handle {
    unsafe {
        if x.is_null() || y.is_null() {
            return ptr::null_mut();
        }
        let node = if (*x).stack == (*y).stack {
            (*y).node.hide();
            (*y).node.clone()
        } else {
            (*y).node.clone_subtree()
        };
        let child = (*x).node.add_child(node);
        handle((*x).stack, child, None)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_insert_cdata(
    x: *mut Handle,
    src: *const c_char,
    len: usize,
) -> *mut Handle {
    unsafe {
        if x.is_null() || src.is_null() {
            return ptr::null_mut();
        }
        let value = valid_text(bytes(src, len));
        if let Some(last) = (*x).node.last_child()
            && last.node_type() == IksType::CData
        {
            let value = last.content().unwrap_or_default() + &value;
            last.set_content(value);
            return handle((*x).stack, last, None);
        }
        let node = (*x).node.insert_cdata(value);
        handle((*x).stack, node, None)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_set_cdata(
    x: *mut Handle,
    src: *const c_char,
    len: usize,
) -> *mut Handle {
    unsafe {
        if x.is_null() || src.is_null() {
            return ptr::null_mut();
        }
        for child in (*x).node.children() {
            child.hide();
        }
        iks_insert_cdata(x, src, len)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_insert_attrib(
    x: *mut Handle,
    name: *const c_char,
    value: *const c_char,
) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let Some(name) = text(name) else {
            return ptr::null_mut();
        };
        if let Some(value) = text(value) {
            (*x).node.add_attribute(&name, value);
            handle((*x).stack, (*x).node.clone(), Some(name))
        } else {
            (*x).node.remove_attribute(&name);
            ptr::null_mut()
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_hide(x: *mut Handle) {
    unsafe {
        if !x.is_null() {
            if let Some(attr) = &(*x).attribute {
                (*x).node.remove_attribute(attr);
            } else {
                (*x).node.hide();
            }
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_delete(x: *mut Handle) {
    unsafe {
        if !x.is_null() {
            iks_stack_delete((*x).stack);
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_stack(x: *mut Handle) -> *mut Stack {
    unsafe {
        if x.is_null() {
            ptr::null_mut()
        } else {
            (*x).stack
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_type(x: *mut Handle) -> c_int {
    unsafe {
        if x.is_null() {
            0
        } else if (*x).attribute.is_some() {
            2
        } else {
            (*x).node.node_type() as c_int
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_name(x: *mut Handle) -> *mut c_char {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let value = (*x).attribute.clone().or_else(|| (*x).node.name());
        value
            .map(|v| string((*x).stack, &v))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_cdata(x: *mut Handle) -> *mut c_char {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let value = if let Some(name) = &(*x).attribute {
            (*x).node.find_attrib(name)
        } else {
            (*x).node.content()
        };
        value
            .map(|v| string((*x).stack, &v))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_cdata_size(x: *mut Handle) -> usize {
    unsafe {
        if x.is_null() {
            0
        } else {
            (*x).node.content().map(|v| v.len()).unwrap_or(0)
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_has_children(x: *mut Handle) -> c_int {
    unsafe { (!x.is_null() && (*x).node.has_children()) as c_int }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_has_attribs(x: *mut Handle) -> c_int {
    unsafe { (!x.is_null() && !(*x).node.borrow().attributes().is_empty()) as c_int }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_string(s: *mut Stack, x: *mut Handle) -> *mut c_char {
    unsafe {
        if x.is_null() {
            ptr::null_mut()
        } else {
            string(s, &(*x).node.to_string())
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_copy(x: *mut Handle) -> *mut Handle {
    unsafe {
        if x.is_null() {
            ptr::null_mut()
        } else {
            new_tree((*x).node.clone_subtree())
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_copy_within(x: *mut Handle, s: *mut Stack) -> *mut Handle {
    unsafe {
        if x.is_null() {
            ptr::null_mut()
        } else if s.is_null() {
            iks_copy(x)
        } else {
            handle(s, (*x).node.clone_subtree(), None)
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_find(x: *mut Handle, name: *const c_char) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        text(name)
            .map(|name| lookup(x, (*x).node.find(&name)))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_find_cdata(x: *mut Handle, name: *const c_char) -> *mut c_char {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        text(name)
            .and_then(|name| (*x).node.find_cdata(&name))
            .map(|v| string((*x).stack, &v))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_find_attrib(x: *mut Handle, name: *const c_char) -> *mut c_char {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        text(name)
            .and_then(|name| (*x).node.find_attrib(&name))
            .map(|v| string((*x).stack, &v))
            .unwrap_or(ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_find_with_attrib(
    x: *mut Handle,
    tag: *const c_char,
    attr: *const c_char,
    value: *const c_char,
) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let (tag, attr, value) = (text(tag), text(attr), text(value));
        if let (Some(attr), Some(value)) = (attr, value) {
            let child = (*x)
                .node
                .borrow()
                .find_with_attrib(tag.as_deref(), &attr, &value)
                .map(NodeRef);
            lookup(x, child)
        } else {
            ptr::null_mut()
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_attrib(x: *mut Handle) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let name = (*x)
            .node
            .borrow()
            .attributes()
            .first()
            .map(|(name, _)| name.clone());
        name.map(|name| handle((*x).stack, (*x).node.clone(), Some(name)))
            .unwrap_or(ptr::null_mut())
    }
}
unsafe fn adjacent(x: *mut Handle, forward: bool) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        if let Some(name) = &(*x).attribute {
            let names: Vec<_> = (*x)
                .node
                .borrow()
                .attributes()
                .iter()
                .map(|(k, _)| k.clone())
                .collect();
            let Some(i) = names.iter().position(|n| n == name) else {
                return ptr::null_mut();
            };
            let next = if forward {
                i.checked_add(1)
            } else {
                i.checked_sub(1)
            };
            return next
                .and_then(|i| names.get(i))
                .map(|n| handle((*x).stack, (*x).node.clone(), Some(n.clone())))
                .unwrap_or(ptr::null_mut());
        }
        lookup(
            x,
            if forward {
                (*x).node.next()
            } else {
                (*x).node.prev()
            },
        )
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_next(x: *mut Handle) -> *mut Handle {
    unsafe { adjacent(x, true) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_prev(x: *mut Handle) -> *mut Handle {
    unsafe { adjacent(x, false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_parent(x: *mut Handle) -> *mut Handle {
    unsafe {
        if x.is_null() {
            ptr::null_mut()
        } else if (*x).attribute.is_some() {
            handle((*x).stack, (*x).node.clone(), None)
        } else {
            lookup(x, (*x).node.parent())
        }
    }
}
macro_rules! node_getter {
    ($name:ident,$method:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(x: *mut Handle) -> *mut Handle {
            unsafe {
                if x.is_null() {
                    ptr::null_mut()
                } else {
                    lookup(x, (*x).node.$method())
                }
            }
        }
    };
}
node_getter!(iks_child, first_child);
node_getter!(iks_first_tag, first_tag);
node_getter!(iks_next_tag, next_tag);
node_getter!(iks_prev_tag, prev_tag);
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_root(x: *mut Handle) -> *mut Handle {
    unsafe {
        if x.is_null() {
            ptr::null_mut()
        } else {
            handle((*x).stack, (*x).node.root(), None)
        }
    }
}
unsafe fn sibling(x: *mut Handle, name: *const c_char, prepend: bool) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let Some(name) = text(name) else {
            return ptr::null_mut();
        };
        let Some(parent) = (*x).node.parent() else {
            return ptr::null_mut();
        };
        let siblings = parent.children();
        let Some(index) = siblings.iter().position(|n| Rc::ptr_eq(&n.0, &(*x).node.0)) else {
            return ptr::null_mut();
        };
        let inserted = NodeRef::new_tag(name);
        let split = if prepend { index } else { index + 1 };
        let tail = siblings[split..].to_vec();
        for node in &tail {
            node.hide();
        }
        parent.add_child(inserted.clone());
        for node in tail {
            parent.add_child(node);
        }
        handle((*x).stack, inserted, None)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_append(x: *mut Handle, name: *const c_char) -> *mut Handle {
    unsafe { sibling(x, name, false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_prepend(x: *mut Handle, name: *const c_char) -> *mut Handle {
    unsafe { sibling(x, name, true) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_insert_sibling(x: *mut Handle, name: *const c_char) -> *mut Handle {
    unsafe { sibling(x, name, false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_append_cdata(
    x: *mut Handle,
    src: *const c_char,
    len: usize,
) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let node = data(src, len).and_then(|v| (*x).node.append_cdata(v));
        lookup(x, node)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_prepend_cdata(
    x: *mut Handle,
    src: *const c_char,
    len: usize,
) -> *mut Handle {
    unsafe {
        if x.is_null() {
            return ptr::null_mut();
        }
        let node = data(src, len).and_then(|v| (*x).node.prepend_cdata(v));
        lookup(x, node)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_base64_encode(src: *const c_char, len: c_int) -> *mut c_char {
    unsafe {
        if len < 0 {
            return ptr::null_mut();
        }
        string(
            ptr::null_mut(),
            &iksemel::base64_encode(bytes(src, len as usize)),
        )
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_base64_decode(src: *const c_char) -> *mut c_char {
    unsafe {
        let Some(src) = text(src) else {
            return ptr::null_mut();
        };
        let Ok(data) = iksemel::base64_decode(&src) else {
            return ptr::null_mut();
        };
        let out = alloc(ptr::null_mut(), data.len() + 1).cast();
        ptr::copy_nonoverlapping(data.as_ptr(), out, data.len());
        out.cast()
    }
}
macro_rules! hash_api {
    ($ctx:ty,$new:ident,$reset:ident,$update:ident,$print:ident,$delete:ident,$one:ident,$hex:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $new() -> *mut $ctx {
            Box::into_raw(Box::new(<$ctx>::new()))
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reset(x: *mut $ctx) {
            unsafe {
                if !x.is_null() {
                    (*x).reset();
                }
            }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $update(x: *mut $ctx, data: *const u8, len: usize, _: c_int) {
            unsafe {
                if !x.is_null() && !data.is_null() {
                    (*x).update(std::slice::from_raw_parts(data, len));
                }
            }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $print(x: *mut $ctx, out: *mut c_char) {
            unsafe {
                if !x.is_null() && !out.is_null() {
                    let text = (*x).hex();
                    ptr::copy_nonoverlapping(text.as_ptr(), out.cast(), text.len());
                    *out.add(text.len()) = 0;
                }
            }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $delete(x: *mut $ctx) {
            unsafe {
                if !x.is_null() {
                    drop(Box::from_raw(x));
                }
            }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $one(src: *const c_char, out: *mut c_char) {
            unsafe {
                if !out.is_null() {
                    let text = $hex(bytes(src, 0));
                    ptr::copy_nonoverlapping(text.as_ptr(), out.cast(), text.len());
                    *out.add(text.len()) = 0;
                }
            }
        }
    };
}
hash_api!(
    iksemel::Md5Context,
    iks_md5_new,
    iks_md5_reset,
    iks_md5_hash,
    iks_md5_print,
    iks_md5_delete,
    iks_md5,
    iksemel::md5_hex
);
hash_api!(
    iksemel::Sha1Context,
    iks_sha_new,
    iks_sha_reset,
    iks_sha_hash,
    iks_sha_print,
    iks_sha_delete,
    iks_sha,
    iksemel::sha1_hex
);
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iks_md5_digest(x: *mut iksemel::Md5Context, out: *mut u8) {
    unsafe {
        if !x.is_null() && !out.is_null() {
            ptr::copy_nonoverlapping((*x).digest().as_ptr(), out, 16);
        }
    }
}
