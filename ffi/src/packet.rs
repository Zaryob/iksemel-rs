use super::*;
#[repr(C)]
pub struct Jid {
    pub user: *mut c_char,
    pub server: *mut c_char,
    pub resource: *mut c_char,
    pub partial: *mut c_char,
    pub full: *mut c_char,
}
#[repr(C)]
pub struct Packet {
    pub x: *mut Handle,
    pub from: *mut Jid,
    pub query: *mut Handle,
    pub ns: *mut c_char,
    pub id: *mut c_char,
    pub kind: c_int,
    pub subtype: c_int,
    pub show: c_int,
}
#[no_mangle]
pub unsafe extern "C" fn iks_id_new(s: *mut Stack, input: *const c_char) -> *mut Jid {
    let Some(input) = text(input) else {
        return ptr::null_mut();
    };
    let jid = PacketJid::new(&input);
    let out = alloc(s, std::mem::size_of::<Jid>()).cast::<Jid>();
    ptr::write(
        out,
        Jid {
            user: jid
                .user
                .as_ref()
                .map(|v| string(s, v))
                .unwrap_or(ptr::null_mut()),
            server: string(s, &jid.server),
            resource: jid
                .resource
                .as_ref()
                .map(|v| string(s, v))
                .unwrap_or(ptr::null_mut()),
            partial: string(s, &jid.partial),
            full: string(s, &jid.full),
        },
    );
    out
}
#[no_mangle]
pub unsafe extern "C" fn iks_id_cmp(a: *mut Jid, b: *mut Jid, parts: c_int) -> c_int {
    if a.is_null() || b.is_null() {
        return 7;
    }
    let mut difference = 0;
    if parts & 1 != 0 && iks_strcasecmp((*a).user, (*b).user) != 0 {
        difference |= 1;
    }
    if parts & 2 != 0 && iks_strcmp((*a).server, (*b).server) != 0 {
        difference |= 2;
    }
    if parts & 4 != 0 && iks_strcmp((*a).resource, (*b).resource) != 0 {
        difference |= 4;
    }
    difference
}
#[no_mangle]
pub unsafe extern "C" fn iks_packet(x: *mut Handle) -> *mut Packet {
    if x.is_null() {
        return ptr::null_mut();
    }
    let s = (*x).stack;
    let pak = iksemel::IksPacket::from_node_ref(&(*x).node);
    let out = alloc(s, std::mem::size_of::<Packet>()).cast::<Packet>();
    ptr::write(
        out,
        Packet {
            x,
            from: pak
                .from
                .as_ref()
                .map(|jid| {
                    let input = string(s, &jid.full);
                    iks_id_new(s, input)
                })
                .unwrap_or(ptr::null_mut()),
            query: pak
                .query
                .map(|q| handle(s, q, None))
                .unwrap_or(ptr::null_mut()),
            ns: pak
                .ns
                .as_ref()
                .map(|v| string(s, v))
                .unwrap_or(ptr::null_mut()),
            id: pak
                .id
                .as_ref()
                .map(|v| string(s, v))
                .unwrap_or(ptr::null_mut()),
            kind: pak.packet_type as c_int,
            subtype: pak.subtype as c_int,
            show: pak.show as c_int,
        },
    );
    out
}
fn subtype(v: c_int) -> iksemel::IksSubtype {
    use iksemel::IksSubtype::*;
    match v {
        1 => Error,
        2 => Chat,
        3 => Groupchat,
        4 => Headline,
        5 => Get,
        6 => Set,
        7 => Result,
        8 => Subscribe,
        9 => Subscribed,
        10 => Unsubscribe,
        11 => Unsubscribed,
        12 => Probe,
        13 => Available,
        14 => Unavailable,
        _ => None,
    }
}
fn show(v: c_int) -> iksemel::IksShowType {
    use iksemel::IksShowType::*;
    match v {
        1 => Available,
        2 => Chat,
        3 => Away,
        4 => Xa,
        5 => Dnd,
        _ => Unavailable,
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_msg(
    kind: c_int,
    to: *const c_char,
    body: *const c_char,
) -> *mut Handle {
    new_tree(
        iksemel::make_message(subtype(kind), text(to).as_deref(), text(body).as_deref()).into(),
    )
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_s10n(
    kind: c_int,
    to: *const c_char,
    body: *const c_char,
) -> *mut Handle {
    new_tree(
        iksemel::make_subscription(subtype(kind), text(to).as_deref(), text(body).as_deref())
            .into(),
    )
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_pres(kind: c_int, status: *const c_char) -> *mut Handle {
    new_tree(iksemel::make_presence(show(kind), text(status).as_deref()).into())
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_iq(kind: c_int, ns: *const c_char) -> *mut Handle {
    new_tree(iksemel::make_iq(subtype(kind), text(ns).as_deref()).into())
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_auth(
    jid: *mut Jid,
    password: *const c_char,
    id: *const c_char,
) -> *mut Handle {
    if jid.is_null() {
        return ptr::null_mut();
    }
    let jid = PacketJid::new(&text((*jid).full).unwrap_or_default());
    new_tree(
        iksemel::make_auth(
            &jid,
            &text(password).unwrap_or_default(),
            text(id).as_deref(),
        )
        .into(),
    )
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_resource_bind(jid: *mut Jid) -> *mut Handle {
    if jid.is_null() {
        return ptr::null_mut();
    }
    new_tree(
        iksemel::make_resource_bind(&PacketJid::new(&text((*jid).full).unwrap_or_default())).into(),
    )
}
#[no_mangle]
pub unsafe extern "C" fn iks_make_session() -> *mut Handle {
    new_tree(iksemel::make_session().into())
}
#[no_mangle]
pub unsafe extern "C" fn iks_stream_features(x: *mut Handle) -> c_int {
    if x.is_null() || (*x).node.name().as_deref() != Some("stream:features") {
        return 0;
    }
    let mut features = 0;
    for child in (*x).node.child_tags() {
        match child.name().as_deref() {
            Some("starttls") => features |= 1,
            Some("session") => features |= 2,
            Some("bind") => features |= 4,
            Some("mechanisms") => {
                for mech in child.child_tags() {
                    match mech.first_child().and_then(|n| n.content()).as_deref() {
                        Some("PLAIN") => features |= 8,
                        Some("DIGEST-MD5") => features |= 16,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    features
}
pub type FilterHook = Option<unsafe extern "C" fn(*mut c_void, *mut Packet) -> c_int>;
#[repr(C)]
pub struct Criterion {
    kind: c_int,
    number: c_int,
    value: *const c_char,
}
pub struct Rule {
    hook: FilterHook,
    user: *mut c_void,
    criteria: Vec<(c_int, c_int, Option<String>)>,
}
#[derive(Default)]
pub struct Filter {
    rules: Vec<Box<Rule>>,
}
#[no_mangle]
pub extern "C" fn iks_filter_new() -> *mut Filter {
    Box::into_raw(Box::default())
}
#[no_mangle]
pub unsafe extern "C" fn iksemel_filter_add_rule_v(
    f: *mut Filter,
    hook: FilterHook,
    user: *mut c_void,
    criteria: *const Criterion,
    count: usize,
) -> *mut Rule {
    if f.is_null() {
        return ptr::null_mut();
    }
    let criteria = std::slice::from_raw_parts(criteria, count)
        .iter()
        .map(|c| (c.kind, c.number, text(c.value)))
        .collect();
    let mut rule = Box::new(Rule {
        hook,
        user,
        criteria,
    });
    let out = &mut *rule as *mut Rule;
    (*f).rules.push(rule);
    out
}
#[no_mangle]
pub unsafe extern "C" fn iks_filter_remove_rule(f: *mut Filter, rule: *mut Rule) {
    if !f.is_null() {
        (*f).rules.retain(|r| !ptr::eq(&**r, rule));
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_filter_remove_hook(f: *mut Filter, hook: FilterHook) {
    if !f.is_null() {
        let key = hook.map(|h| h as usize);
        (*f).rules.retain(|r| r.hook.map(|h| h as usize) != key);
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_filter_packet(f: *mut Filter, p: *mut Packet) {
    if f.is_null() || p.is_null() {
        return;
    }
    let mut matches = Vec::new();
    for rule in &(*f).rules {
        let mut score = 0;
        let mut failed = false;
        for (kind, number, value) in &rule.criteria {
            let (matched, weight) = match kind {
                2 => ((*p).kind == *number, 1),
                4 => ((*p).subtype == *number, 2),
                1 => (text((*p).id) == *value, 16),
                32 => (text((*p).ns) == *value, 4),
                8 => (!(*p).from.is_null() && text((*(*p).from).full) == *value, 8),
                16 => (
                    !(*p).from.is_null() && text((*(*p).from).partial) == *value,
                    8,
                ),
                _ => (false, 0),
            };
            if matched {
                score += weight
            } else {
                failed = true;
            }
        }
        if !failed && score > 0 {
            matches.push((score, &**rule as *const Rule));
        }
    }
    matches.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    for (_, rule) in matches {
        if let Some(hook) = (*rule).hook {
            if hook((*rule).user, p) == 1 {
                break;
            }
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn iks_filter_delete(f: *mut Filter) {
    if !f.is_null() {
        drop(Box::from_raw(f));
    }
}
