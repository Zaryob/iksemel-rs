use iksemel::{
    Connection, FilterHook, FilterStatus, IksNode, IksPacket, IksPacketType, IksShowType,
    IksSubtype, Jid, PacketFilter, RuleBuilder, RuleId,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// C Parity Criterion 1: Weighted scoring priority order
/// ID = 16, FROM = 8, FROM_PARTIAL = 8, NS = 4, SUBTYPE = 2, TYPE = 1
#[test]
fn test_c_parity_weighted_scoring_order() {
    let mut filter = PacketFilter::new();
    let order = Arc::new(Mutex::new(Vec::new()));

    // Rule A: id (16)
    let o_a = order.clone();
    filter.add_rule(RuleBuilder::new().with_id("req_1"), move |_| {
        o_a.lock().unwrap().push("id_16");
        FilterStatus::Pass
    });

    // Rule B: from full (8)
    let o_b = order.clone();
    filter.add_rule(
        RuleBuilder::new().with_from(Jid::new("bot@domain.com/res1").unwrap()),
        move |_| {
            o_b.lock().unwrap().push("from_8");
            FilterStatus::Pass
        },
    );

    // Rule C: ns (4)
    let o_c = order.clone();
    filter.add_rule(RuleBuilder::new().with_ns("jabber:iq:version"), move |_| {
        o_c.lock().unwrap().push("ns_4");
        FilterStatus::Pass
    });

    // Rule D: subtype (2)
    let o_d = order.clone();
    filter.add_rule(
        RuleBuilder::new().with_subtype(IksSubtype::Get),
        move |_| {
            o_d.lock().unwrap().push("subtype_2");
            FilterStatus::Pass
        },
    );

    // Rule E: type (1)
    let o_e = order.clone();
    filter.add_rule(RuleBuilder::new().with_type(IksPacketType::Iq), move |_| {
        o_e.lock().unwrap().push("type_1");
        FilterStatus::Pass
    });

    // Rule F: compound id + ns (16 + 4 = 20)
    let o_f = order.clone();
    filter.add_rule(
        RuleBuilder::new()
            .with_id("req_1")
            .with_ns("jabber:iq:version"),
        move |_| {
            o_f.lock().unwrap().push("compound_20");
            FilterStatus::Pass
        },
    );

    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("id", "req_1");
    iq.add_attribute("type", "get");
    iq.add_attribute("from", "bot@domain.com/res1");
    let mut q = IksNode::new_tag("query");
    q.add_attribute("xmlns", "jabber:iq:version");
    iq.add_child(q);

    let pak = IksPacket::from_node(&iq);
    let status = filter.filter_packet(&pak);
    assert_eq!(status, FilterStatus::Pass);

    let actual_order = order.lock().unwrap().clone();
    assert_eq!(
        actual_order,
        vec![
            "compound_20",
            "id_16",
            "from_8",
            "ns_4",
            "subtype_2",
            "type_1"
        ]
    );
}

/// C Parity Criterion 2: FilterStatus::Eat immediately terminates dispatch
#[test]
fn test_c_parity_filter_eat_terminates_dispatch() {
    let mut filter = PacketFilter::new();
    let executed = Arc::new(Mutex::new(Vec::new()));

    // High score rule that eats
    let ex1 = executed.clone();
    filter.add_rule(RuleBuilder::new().with_id("eat_me"), move |_| {
        ex1.lock().unwrap().push("high_score_eat");
        FilterStatus::Eat
    });

    // Low score rule that should NOT run
    let ex2 = executed.clone();
    filter.add_rule(RuleBuilder::new().with_type(IksPacketType::Iq), move |_| {
        ex2.lock().unwrap().push("low_score_pass");
        FilterStatus::Pass
    });

    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("id", "eat_me");
    let pak = IksPacket::from_node(&iq);

    let status = filter.filter_packet(&pak);
    assert_eq!(status, FilterStatus::Eat);

    let run_list = executed.lock().unwrap().clone();
    assert_eq!(run_list, vec!["high_score_eat"]);
}

/// C Parity Criterion 3: ns is ONLY matched for Iq and ONLY from the first tag child carrying an xmlns attribute (jabber.c:146-155)
#[test]
fn test_c_parity_ns_from_first_iq_child_with_xmlns() {
    let mut filter = PacketFilter::new();
    let ns_ran = Arc::new(AtomicUsize::new(0));

    let ran_c = ns_ran.clone();
    filter.add_rule(RuleBuilder::new().with_ns("custom:ns"), move |_| {
        ran_c.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });

    // 1. Message with xmlns child must NOT match
    let mut msg = IksNode::new_tag("message");
    let mut body = IksNode::new_tag("body");
    body.add_attribute("xmlns", "custom:ns");
    msg.add_child(body);
    let pak_msg = IksPacket::from_node(&msg);
    filter.filter_packet(&pak_msg);
    assert_eq!(ns_ran.load(Ordering::SeqCst), 0);

    // 2. Presence with xmlns child must NOT match
    let mut pres = IksNode::new_tag("presence");
    let mut x = IksNode::new_tag("x");
    x.add_attribute("xmlns", "custom:ns");
    pres.add_child(x);
    let pak_pres = IksPacket::from_node(&pres);
    filter.filter_packet(&pak_pres);
    assert_eq!(ns_ran.load(Ordering::SeqCst), 0);

    // 3. IQ with second child having xmlns (first is cdata) must still find first TAG child with xmlns
    let mut iq = IksNode::new_tag("iq");
    iq.add_child(IksNode::new_cdata("ignored"));
    let mut q = IksNode::new_tag("query");
    q.add_attribute("xmlns", "custom:ns");
    iq.add_child(q);
    let pak_iq = IksPacket::from_node(&iq);
    filter.filter_packet(&pak_iq);
    assert_eq!(ns_ran.load(Ordering::SeqCst), 1);

    // 4. IQ whose first tag child has no xmlns: ns comes from the next tag child that has one
    let mut iq = IksNode::new_tag("iq");
    iq.add_child(IksNode::new_tag("first"));
    let mut q = IksNode::new_tag("query");
    q.add_attribute("xmlns", "custom:ns");
    iq.add_child(q);
    let pak_iq = IksPacket::from_node(&iq);
    assert_eq!(pak_iq.ns.as_deref(), Some("custom:ns"));
    assert_eq!(
        pak_iq.query.as_ref().and_then(|q| q.name()).as_deref(),
        Some("query")
    );
    filter.filter_packet(&pak_iq);
    assert_eq!(ns_ran.load(Ordering::SeqCst), 2);
}

/// C Parity Criterion 4: Subscription (IKS_PAK_S10N) vs Presence classification
#[test]
fn test_c_parity_subscription_vs_presence() {
    // presence type="probe" -> Presence, Probe
    let mut probe = IksNode::new_tag("presence");
    probe.add_attribute("type", "probe");
    let p_probe = IksPacket::from_node(&probe);
    assert_eq!(p_probe.packet_type, IksPacketType::Presence);
    assert_eq!(p_probe.subtype, IksSubtype::Probe);

    // presence type="unavailable" -> Presence, Unavailable
    let mut unavail = IksNode::new_tag("presence");
    unavail.add_attribute("type", "unavailable");
    let p_unavail = IksPacket::from_node(&unavail);
    assert_eq!(p_unavail.packet_type, IksPacketType::Presence);
    assert_eq!(p_unavail.subtype, IksSubtype::Unavailable);
    assert_eq!(p_unavail.show, IksShowType::Unavailable);

    // presence type="subscribe" -> Subscription, Subscribe
    let mut sub = IksNode::new_tag("presence");
    sub.add_attribute("type", "subscribe");
    let p_sub = IksPacket::from_node(&sub);
    assert_eq!(p_sub.packet_type, IksPacketType::Subscription);
    assert_eq!(p_sub.subtype, IksSubtype::Subscribe);

    // presence type="subscribed" -> Subscription, Subscribed
    let mut subd = IksNode::new_tag("presence");
    subd.add_attribute("type", "subscribed");
    let p_subd = IksPacket::from_node(&subd);
    assert_eq!(p_subd.packet_type, IksPacketType::Subscription);
    assert_eq!(p_subd.subtype, IksSubtype::Subscribed);

    // presence type="unsubscribe" -> Subscription, Unsubscribe
    let mut unsub = IksNode::new_tag("presence");
    unsub.add_attribute("type", "unsubscribe");
    let p_unsub = IksPacket::from_node(&unsub);
    assert_eq!(p_unsub.packet_type, IksPacketType::Subscription);
    assert_eq!(p_unsub.subtype, IksSubtype::Unsubscribe);

    // presence type="unsubscribed" -> Subscription, Unsubscribed
    let mut unsubd = IksNode::new_tag("presence");
    unsubd.add_attribute("type", "unsubscribed");
    let p_unsubd = IksPacket::from_node(&unsubd);
    assert_eq!(p_unsubd.packet_type, IksPacketType::Subscription);
    assert_eq!(p_unsubd.subtype, IksSubtype::Unsubscribed);
}

/// C Parity Criterion 5: from (full) vs from_partial (bare) matching
#[test]
fn test_c_parity_from_and_from_partial() {
    let mut filter = PacketFilter::new();
    let ran_full = Arc::new(AtomicUsize::new(0));
    let ran_partial = Arc::new(AtomicUsize::new(0));

    let f_c = ran_full.clone();
    filter.add_rule(
        RuleBuilder::new().with_from(Jid::new("alice@example.com/phone").unwrap()),
        move |_| {
            f_c.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        },
    );

    let p_c = ran_partial.clone();
    filter.add_rule(
        RuleBuilder::new().with_from_partial(Jid::new("alice@example.com").unwrap()),
        move |_| {
            p_c.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        },
    );

    // Incoming packet from "alice@example.com/desktop"
    let mut msg1 = IksNode::new_tag("message");
    msg1.add_attribute("from", "alice@example.com/desktop");
    let pak1 = IksPacket::from_node(&msg1);
    filter.filter_packet(&pak1);

    // Only from_partial matches; full does not match different resource!
    assert_eq!(ran_full.load(Ordering::SeqCst), 0);
    assert_eq!(ran_partial.load(Ordering::SeqCst), 1);

    // Incoming packet from "alice@example.com/phone"
    let mut msg2 = IksNode::new_tag("message");
    msg2.add_attribute("from", "alice@example.com/phone");
    let pak2 = IksPacket::from_node(&msg2);
    filter.filter_packet(&pak2);

    // Both match now!
    assert_eq!(ran_full.load(Ordering::SeqCst), 1);
    assert_eq!(ran_partial.load(Ordering::SeqCst), 2);
}

/// C Parity Criterion 6: remove_rule dynamically alters dispatch behavior
#[test]
fn test_c_parity_remove_rule() {
    let mut filter = PacketFilter::new();
    let ran_count = Arc::new(AtomicUsize::new(0));

    let r_c = ran_count.clone();
    let rule_id: RuleId =
        filter.add_rule(RuleBuilder::new().with_id("rule_to_remove"), move |_| {
            r_c.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        });

    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("id", "rule_to_remove");
    let pak = IksPacket::from_node(&iq);

    assert_eq!(filter.filter_packet(&pak), FilterStatus::Pass);
    assert_eq!(ran_count.load(Ordering::SeqCst), 1);

    assert!(filter.remove_rule(rule_id));
    assert!(!filter.remove_rule(rule_id)); // Already removed

    // Must not trigger anymore
    assert_eq!(filter.filter_packet(&pak), FilterStatus::Pass);
    assert_eq!(ran_count.load(Ordering::SeqCst), 1);
}

/// C Parity Criterion 7: recv_iq_response ID matching during real connection exchange
#[test]
fn test_c_parity_connection_iq_correlation_under_load() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];

        // 1. Initial stream header
        let _ = socket.read(&mut buf).unwrap();
        let server_hdr = "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>";
        socket.write_all(server_hdr.as_bytes()).unwrap();

        // 2. Read client request
        let _ = socket.read(&mut buf).unwrap();

        // 3. Flood with diverse interleaved stanzas before sending target IQ
        let flood = concat!(
            "<presence from='contact1@domain.com'><show>away</show></presence>",
            "<message from='contact2@domain.com' type='chat'><body>hello 1</body></message>",
            "<presence from='contact3@domain.com'><show>dnd</show></presence>",
            "<iq id='unrelated_iq' type='get'><ping xmlns='urn:xmpp:ping'/></iq>",
            "<message from='contact4@domain.com' type='chat'><body>hello 2</body></message>",
            "<iq id='correlated_response_99' type='result'><query xmlns='jabber:iq:roster'/></iq>",
            "<presence from='contact5@domain.com'/>"
        );
        socket.write_all(flood.as_bytes()).unwrap();
    });

    let mut conn = Connection::connect(
        "127.0.0.1",
        port,
        "example.com",
        Some(Duration::from_secs(5)),
    )
    .unwrap();
    conn.start_stream().unwrap();

    let mut probe_iq = IksNode::new_tag("iq");
    probe_iq.add_attribute("id", "correlated_response_99");
    probe_iq.add_attribute("type", "get");
    conn.send_stanza(&probe_iq).unwrap();

    // Must correlate with target IQ without being distracted by 5 interleaved messages and presences
    let iq_resp = conn
        .recv_iq_response("correlated_response_99")
        .expect("correlated IQ received");
    assert_eq!(
        iq_resp.find_attrib("id").as_deref(),
        Some("correlated_response_99")
    );
    assert_eq!(iq_resp.find_attrib("type").as_deref(), Some("result"));

    // Verify all 5 preceding interleaved stanzas were retained in perfect order!
    let s1 = conn.recv_stanza().unwrap();
    assert_eq!(
        s1.find_attrib("from").as_deref(),
        Some("contact1@domain.com")
    );

    let s2 = conn.recv_stanza().unwrap();
    assert_eq!(
        s2.find_attrib("from").as_deref(),
        Some("contact2@domain.com")
    );

    let s3 = conn.recv_stanza().unwrap();
    assert_eq!(
        s3.find_attrib("from").as_deref(),
        Some("contact3@domain.com")
    );

    let s4 = conn.recv_stanza().unwrap();
    assert_eq!(s4.find_attrib("id").as_deref(), Some("unrelated_iq"));

    let s5 = conn.recv_stanza().unwrap();
    assert_eq!(
        s5.find_attrib("from").as_deref(),
        Some("contact4@domain.com")
    );

    let s6 = conn.recv_stanza().unwrap();
    assert_eq!(
        s6.find_attrib("from").as_deref(),
        Some("contact5@domain.com")
    );

    server_handle.join().unwrap();
}

/// C Parity Criterion 8: dispatch backward compatibility count
#[test]
fn test_c_parity_dispatch_legacy_compat() {
    let mut filter = PacketFilter::new();

    filter.add_rule(RuleBuilder::new().with_id("compat_1"), |_| {
        FilterStatus::Pass
    });
    filter.add_rule(RuleBuilder::new().with_id("compat_1"), |_| {
        FilterStatus::Pass
    });

    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("id", "compat_1");

    let count = filter.dispatch(&iq);
    assert_eq!(count, 2);
}

fn msg_from(from: &str) -> IksPacket {
    let mut msg = IksNode::new_tag("message");
    msg.add_attribute("from", from);
    IksPacket::from_node(&msg)
}

/// Counts how often a single-rule filter fires for `from`.
fn from_rule_fires(rule: RuleBuilder, from: &str) -> usize {
    let mut filter = PacketFilter::new();
    let ran = Arc::new(AtomicUsize::new(0));
    let r = ran.clone();
    filter.add_rule(rule, move |_| {
        r.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });
    filter.filter_packet(&msg_from(from));
    ran.load(Ordering::SeqCst)
}

/// C oracle case 87: iks_id_new keeps case; matching is byte-exact (iks_strcmp).
#[test]
fn test_c_parity_from_is_case_sensitive_and_raw() {
    let from = "Bob@Example.COM/phone";
    let pak = msg_from(from);
    let pj = pak.from.as_ref().unwrap();
    assert_eq!(pj.full, "Bob@Example.COM/phone");
    assert_eq!(pj.partial, "Bob@Example.COM");
    assert_eq!(pj.user.as_deref(), Some("Bob"));
    assert_eq!(pj.server, "Example.COM");
    assert_eq!(pj.resource.as_deref(), Some("phone"));

    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from("bob@example.com/phone"), from),
        0
    );
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from_partial("bob@example.com"),
            from
        ),
        0
    );
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from("Bob@Example.COM/phone"), from),
        1
    );
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from_partial("Bob@Example.COM"),
            from
        ),
        1
    );
}

/// C oracle case 88: the "jabber:" scheme prefix is skipped by iks_id_new.
#[test]
fn test_c_parity_from_strips_jabber_prefix() {
    let from = "jabber:bob@example.com/phone";
    let pak = msg_from(from);
    let pj = pak.from.as_ref().unwrap();
    assert_eq!(pj.full, "bob@example.com/phone");
    assert_eq!(pj.partial, "bob@example.com");
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from("bob@example.com/phone"), from),
        1
    );
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from_partial("bob@example.com"),
            from
        ),
        1
    );
}

/// C oracle cases 89-92: empty/odd addresses are kept raw, never rejected.
#[test]
fn test_c_parity_from_odd_addresses_match_c_oracle() {
    // case 89: from='' -> from present, full == partial == ""
    let pak = msg_from("");
    let pj = pak.from.as_ref().expect("empty from is still Some in C");
    assert_eq!((pj.full.as_str(), pj.partial.as_str()), ("", ""));
    assert_eq!(pj.user, None);
    assert_eq!(pj.resource, None);
    assert_eq!(from_rule_fires(RuleBuilder::new().with_from(""), ""), 1);

    // case 90: '@example.com' -> empty (not absent) user
    let pak = msg_from("@example.com");
    let pj = pak.from.as_ref().unwrap();
    assert_eq!(pj.user.as_deref(), Some(""));
    assert_eq!(pj.server, "example.com");
    assert_eq!(pj.full, "@example.com");
    assert_eq!(pj.partial, "@example.com");
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from("@example.com"), "@example.com"),
        1
    );

    // case 91: trailing '/' -> empty resource, partial excludes the slash
    let pak = msg_from("bob@example.com/");
    let pj = pak.from.as_ref().unwrap();
    assert_eq!(pj.full, "bob@example.com/");
    assert_eq!(pj.partial, "bob@example.com");
    assert_eq!(pj.resource.as_deref(), Some(""));
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from("bob@example.com/"),
            "bob@example.com/"
        ),
        1
    );
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from_partial("bob@example.com"),
            "bob@example.com/"
        ),
        1
    );

    // case 92: '/' is split before '@', so the resource may contain '@'
    let from = "example.com/resource@device";
    let pak = msg_from(from);
    let pj = pak.from.as_ref().unwrap();
    assert_eq!(pj.user, None);
    assert_eq!(pj.server, "example.com");
    assert_eq!(pj.resource.as_deref(), Some("resource@device"));
    assert_eq!(from_rule_fires(RuleBuilder::new().with_from(from), from), 1);
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from_partial("example.com"), from),
        1
    );
}

/// A rule without a `from` attribute on the packet never matches a FROM rule.
#[test]
fn test_c_parity_from_rule_requires_from_attribute() {
    let mut filter = PacketFilter::new();
    let ran = Arc::new(AtomicUsize::new(0));
    let r = ran.clone();
    filter.add_rule(RuleBuilder::new().with_from("a@b"), move |_| {
        r.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });
    let pak = IksPacket::from_node(&IksNode::new_tag("message"));
    assert!(pak.from.is_none());
    filter.filter_packet(&pak);
    assert_eq!(ran.load(Ordering::SeqCst), 0);
}

/// Backward compatibility: `Jid` values are still accepted by with_from/with_from_partial.
#[test]
fn test_c_parity_rule_accepts_jid_values() {
    let from = "alice@example.com/phone";
    let jid = || Jid::new(from).unwrap();
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from(jid()), from),
        1
    );
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from_partial(jid()), from),
        1
    );
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from(jid()),
            "alice@example.com/tablet"
        ),
        0
    );
    assert_eq!(
        from_rule_fires(
            RuleBuilder::new().with_from_partial(jid()),
            "alice@example.com/tablet"
        ),
        1
    );
    // String and &str are accepted too
    assert_eq!(
        from_rule_fires(RuleBuilder::new().with_from(from.to_string()), from),
        1
    );
}

/// iks_filter_remove_hook: removes every rule registered with the same hook.
#[test]
fn test_c_parity_remove_hook() {
    let mut filter = PacketFilter::new();
    let shared_count = Arc::new(AtomicUsize::new(0));
    let other_count = Arc::new(AtomicUsize::new(0));
    let plain_count = Arc::new(AtomicUsize::new(0));

    let sc = shared_count.clone();
    let shared = FilterHook::new(move |_| {
        sc.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });
    let oc = other_count.clone();
    let other = FilterHook::new(move |_| {
        oc.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });
    let pc = plain_count.clone();
    filter.add_rule(
        RuleBuilder::new().with_type(IksPacketType::Message),
        move |_| {
            pc.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        },
    );
    let r1 = filter.add_rule_with_hook(
        RuleBuilder::new().with_type(IksPacketType::Message),
        &shared,
    );
    filter.add_rule_with_hook(RuleBuilder::new().with_subtype(IksSubtype::Chat), &shared);
    filter.add_rule_with_hook(RuleBuilder::new().with_id("x"), &shared);
    filter.add_rule_with_hook(RuleBuilder::new().with_type(IksPacketType::Message), &other);

    // Single removal by RuleId still works for hook-registered rules.
    assert!(filter.remove_rule(r1));
    assert!(!filter.remove_rule(r1));

    let mut msg = IksNode::new_tag("message");
    msg.add_attribute("type", "chat");
    msg.add_attribute("id", "x");
    let pak = IksPacket::from_node(&msg);

    // Pass continues down the score list, so both remaining shared rules run (plus plain/other).
    filter.filter_packet(&pak);
    assert_eq!(shared_count.load(Ordering::SeqCst), 2);

    assert_eq!(filter.remove_hook(&shared), 2);
    assert_eq!(filter.remove_hook(&shared), 0);

    filter.filter_packet(&pak);
    assert_eq!(
        shared_count.load(Ordering::SeqCst),
        2,
        "removed hook must not run again"
    );
    // other + plain rules remain and each ran once per dispatch.
    assert_eq!(plain_count.load(Ordering::SeqCst), 2);
    assert_eq!(other_count.load(Ordering::SeqCst), 2);
    assert_eq!(filter.remove_hook(&other), 1);
    // The add_rule registration is unaffected by remove_hook.
    filter.filter_packet(&pak);
    assert_eq!(plain_count.load(Ordering::SeqCst), 3);
    assert_eq!(other_count.load(Ordering::SeqCst), 2);
}

/// remove_hook with three rules on one hook returns 3 and leaves a different hook intact.
#[test]
fn test_c_parity_remove_hook_returns_count_and_keeps_other_hook() {
    let mut filter = PacketFilter::new();
    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));
    let ac = a.clone();
    let hook_a = FilterHook::new(move |_| {
        ac.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });
    let bc = b.clone();
    let hook_b = FilterHook::new(move |_| {
        bc.fetch_add(1, Ordering::SeqCst);
        FilterStatus::Pass
    });
    filter.add_rule_with_hook(RuleBuilder::new().with_id("1"), &hook_a);
    filter.add_rule_with_hook(RuleBuilder::new().with_id("2"), &hook_a);
    filter.add_rule_with_hook(RuleBuilder::new().with_id("3"), &hook_a);
    filter.add_rule_with_hook(
        RuleBuilder::new().with_type(IksPacketType::Message),
        &hook_b,
    );

    assert_eq!(filter.remove_hook(&hook_a), 3);
    assert_eq!(filter.remove_hook(&hook_a), 0);

    let mut msg = IksNode::new_tag("message");
    msg.add_attribute("id", "1");
    assert_eq!(filter.dispatch(&msg), 1);
    assert_eq!(a.load(Ordering::SeqCst), 0);
    assert_eq!(b.load(Ordering::SeqCst), 1);
}
