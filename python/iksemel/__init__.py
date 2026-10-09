"""Python 3 binding for the Rust-backed iksemel C ABI.

Set IKSEMEL_LIBRARY to a built libiksemel shared library. Nodes retain their
owning document; handles are confined to the thread that created them.
"""
import ctypes as _c
import os as _os
from pathlib import Path as _Path
import threading as _threading

TAG, ATTRIBUTE, DATA = 1, 2, 3
class ParseError(ValueError): pass
class NotTag(TypeError): pass
class NotData(TypeError): pass

def _library():
    specified = _os.environ.get('IKSEMEL_LIBRARY')
    if specified:
        return _c.CDLL(specified)
    root = _Path(__file__).resolve().parents[2]
    for profile in ('release', 'debug'):
        for name in ('libiksemel.dylib', 'libiksemel.so'):
            candidate = root / 'target' / profile / name
            if candidate.exists():
                return _c.CDLL(str(candidate))
    raise ImportError('Build the C library with scripts/build-c-library.py or set IKSEMEL_LIBRARY')
_lib = _library()
_P, _S, _Z, _I = _c.c_void_p, _c.c_char_p, _c.c_size_t, _c.c_int

def _bind(name, result, *args):
    fn = getattr(_lib, 'iks_' + name)
    fn.restype, fn.argtypes = result, list(args)
    return fn
_new = _bind('new', _P, _S)
_tree = _bind('tree', _P, _S, _Z, _c.POINTER(_I))
_delete = _bind('delete', None, _P)
_free = _bind('free', None, _P)
_type = _bind('type', _I, _P)
_name = _bind('name', _S, _P)
_data = _bind('cdata', _S, _P)
_string = _bind('string', _P, _P, _P)
_set_data = _bind('set_cdata', _P, _P, _S, _Z)
_set_attr = _bind('insert_attrib', _P, _P, _S, _S)
_find_attr = _bind('find_attrib', _S, _P, _S)
_find_data = _bind('find_cdata', _S, _P, _S)
_find = _bind('find', _P, _P, _S)
_insert_node = _bind('insert_node', _P, _P, _P)
_hide = _bind('hide', None, _P)
_nav = {n: _bind(n, _P, _P) for n in ('child', 'parent', 'root', 'next', 'prev', 'first_tag', 'next_tag', 'prev_tag', 'attrib')}
_tags = {n: _bind(n, _P, _P, _S) for n in ('insert', 'append', 'prepend')}
_text = {n: _bind(n, _P, _P, _S, _Z) for n in ('insert_cdata', 'append_cdata', 'prepend_cdata')}

def _encode(value):
    if not isinstance(value, str):
        raise TypeError('expected str')
    if '\0' in value:
        raise ValueError('NUL is not permitted in XML strings')
    return value.encode('utf-8')
def _decode(value):
    return None if value is None else value.decode('utf-8')

class Document:
    def __init__(self, pointer):
        if not pointer:
            raise MemoryError('cannot allocate document')
        self._pointer = pointer
        self._thread = _threading.get_ident()
    def _check(self):
        if _threading.get_ident() != self._thread:
            raise RuntimeError('iksemel handles are confined to their creating thread')
        if not self._pointer:
            raise ReferenceError('document is closed')
    def close(self):
        self._check()
        _delete(self._pointer)
        self._pointer = None
    def __del__(self):
        if getattr(self, '_pointer', None) and self._thread == _threading.get_ident():
            _delete(self._pointer)
            self._pointer = None

class Node:
    def __init__(self, pointer, document):
        self._pointer, self._document = pointer, document
    def _check(self, kind=None):
        self._document._check()
        if kind is not None and _type(self._pointer) != kind:
            raise NotTag() if kind == TAG else NotData()
        return self._pointer
    def _wrap(self, pointer):
        return Node(pointer, self._document) if pointer else None
    def type(self): return _type(self._check())
    def name(self): return _decode(_name(self._check(TAG)))
    def data(self): return _decode(_data(self._check(DATA)))
    def setData(self, value): _set_data(self._check(TAG), _encode(value), 0)
    def attributes(self):
        pointer = _nav['attrib'](self._check(TAG))
        names = []
        while pointer:
            names.append(_decode(_name(pointer)))
            pointer = _nav['next'](pointer)
        return names
    def getAttribute(self, name): return _decode(_find_attr(self._check(TAG), _encode(name)))
    def setAttribute(self, name, value):
        _set_attr(self._check(TAG), _encode(name), None if value is None else _encode(value))
    def getTag(self, name): return self._wrap(_find(self._check(TAG), _encode(name)))
    def getTagData(self, name): return _decode(_find_data(self._check(TAG), _encode(name)))
    def firstChild(self): return self._wrap(_nav['child'](self._check(TAG)))
    def parent(self): return self._wrap(_nav['parent'](self._check()))
    def root(self): return self._wrap(_nav['root'](self._check()))
    def next(self): return self._wrap(_nav['next'](self._check()))
    def previous(self): return self._wrap(_nav['prev'](self._check()))
    def _sibling_tag(self, direction, name):
        pointer = _nav[direction](self._check())
        while pointer and name is not None and _decode(_name(pointer)) != name:
            pointer = _nav[direction](pointer)
        return self._wrap(pointer)
    def nextTag(self, name=None): return self._sibling_tag('next_tag', name)
    def previousTag(self, name=None): return self._sibling_tag('prev_tag', name)
    def __iter__(self):
        node = self.firstChild()
        while node is not None:
            following = node.next()
            yield node
            node = following
    def tags(self, name=None):
        self._check(TAG)
        for node in self:
            if node.type() == TAG and (name is None or node.name() == name):
                yield node
    def toString(self):
        pointer = _string(None, self._check())
        if not pointer:
            raise MemoryError()
        try: return _c.string_at(pointer).decode('utf-8')
        finally: _free(pointer)
    def toPrettyString(self):
        # Preserve mixed-content whitespace. Only indent element-only content.
        from xml.dom import minidom
        doc = minidom.parseString(self.toString())
        def render(node, depth):
            if node.nodeType != node.ELEMENT_NODE:
                return node.toxml()
            if not node.childNodes or any(n.nodeType == n.TEXT_NODE for n in node.childNodes):
                return node.toxml()
            opening = node.cloneNode(False).toxml()[:-2] + '>'
            children = '\n'.join('  ' * (depth + 1) + render(n, depth + 1) for n in node.childNodes)
            return opening + '\n' + children + '\n' + '  ' * depth + '</' + node.tagName + '>'
        return render(doc.documentElement, 0) + '\n'
    def insertTag(self, name): return self._wrap(_tags['insert'](self._check(TAG), _encode(name)))
    def appendTag(self, name): return self._wrap(_tags['append'](self._check(), _encode(name)))
    def prependTag(self, name): return self._wrap(_tags['prepend'](self._check(), _encode(name)))
    def insertData(self, value): return self._wrap(_text['insert_cdata'](self._check(TAG), _encode(value), 0))
    def appendData(self, value): return self._wrap(_text['append_cdata'](self._check(), _encode(value), 0))
    def prependData(self, value): return self._wrap(_text['prepend_cdata'](self._check(), _encode(value), 0))
    def insertNode(self, node):
        if not isinstance(node, Node): raise TypeError('expected Node')
        return self._wrap(_insert_node(self._check(TAG), node._check()))
    def hide(self): _hide(self._check())
    def __reduce__(self): return (parseString, (self.toString(),))

def newDocument(name):
    pointer = _new(_encode(name))
    return Node(pointer, Document(pointer))
def parseString(value):
    encoded = _encode(value)
    error = _I()
    pointer = _tree(encoded, len(encoded), _c.byref(error))
    if not pointer or error.value:
        if pointer: _delete(pointer)
        raise ParseError('invalid or incomplete XML')
    return Node(pointer, Document(pointer))
def parse(filename):
    return parseString(_Path(filename).read_text(encoding='utf-8'))
