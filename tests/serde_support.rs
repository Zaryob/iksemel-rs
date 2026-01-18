#![cfg(feature = "serde")]

use iksemel::{DomParser, IksNode, Jid, Roster, RosterItem, SubscriptionType};

#[test]
fn test_jid_serde_json_roundtrip() {
    let jid = Jid::new("alice@example.com/mobile").expect("valid jid");
    let json = serde_json::to_string(&jid).expect("serialize jid");
    assert_eq!(json, "\"alice@example.com/mobile\"");

    let deserialized: Jid = serde_json::from_str(&json).expect("deserialize jid");
    assert_eq!(deserialized, jid);
    assert_eq!(deserialized.node(), Some("alice"));
    assert_eq!(deserialized.domain(), "example.com");
    assert_eq!(deserialized.resource(), Some("mobile"));
}

#[test]
fn test_node_serde_json_roundtrip() {
    let xml = r#"<message from="alice@example.com" to="bob@example.com" type="chat"><body>Hello via JSON!</body></message>"#;
    let original = DomParser::parse_str(xml).expect("parse xml");
    let node: IksNode = original.borrow().clone();

    let json = serde_json::to_string(&node).expect("serialize node");
    assert!(json.contains("message"));
    assert!(json.contains("Hello via JSON!"));

    let deserialized: IksNode = serde_json::from_str(&json).expect("deserialize node");
    assert_eq!(deserialized.name(), Some("message"));
    assert_eq!(deserialized.find_attrib("type"), Some("chat"));
    assert_eq!(deserialized.find_attrib("from"), Some("alice@example.com"));
    assert_eq!(deserialized.find_path_text(&["body"]), Some("Hello via JSON!".to_string()));
}

#[test]
fn test_roster_serde_json_roundtrip() {
    let mut roster = Roster::new();
    let mut item1 = RosterItem::new(Jid::new("friend@example.com").unwrap());
    item1.name = Some("Best Friend".to_string());
    item1.subscription = SubscriptionType::Both;
    item1.groups.push("Close Friends".to_string());

    roster.items.push(item1);

    let json = serde_json::to_string_pretty(&roster).expect("serialize roster");
    assert!(json.contains("friend@example.com"));
    assert!(json.contains("Best Friend"));
    assert!(json.contains("Both"));

    let deserialized: Roster = serde_json::from_str(&json).expect("deserialize roster");
    assert_eq!(deserialized.items.len(), 1);
    assert_eq!(deserialized.items[0].jid.bare(), "friend@example.com");
    assert_eq!(deserialized.items[0].name.as_deref(), Some("Best Friend"));
    assert_eq!(deserialized.items[0].subscription, SubscriptionType::Both);
    assert_eq!(deserialized.items[0].groups, vec!["Close Friends"]);
}
