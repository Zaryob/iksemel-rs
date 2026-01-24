use iksemel::DomParser;

const ATOM_FEED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Example Feed</title>
  <subtitle>A sample feed for selector testing</subtitle>
  <link href="http://example.org/"/>
  <updated>2026-01-15T14:00:00Z</updated>
  <author>
    <name>John Doe</name>
    <email>johndoe@example.com</email>
  </author>
  <entry id="e1" status="published">
    <title>First Article</title>
    <category term="technology" scheme="http://example.org/categories"/>
    <summary>Introductory post about technology.</summary>
    <author>
      <name>Alice Smith</name>
    </author>
  </entry>
  <entry id="e2" status="draft">
    <title>Draft Notes</title>
    <category term="drafts" scheme="http://example.org/categories"/>
    <summary>Draft content not yet public.</summary>
    <author>
      <name>Bob Johnson</name>
    </author>
  </entry>
  <entry id="e3" status="published">
    <title>Advanced Rust Systems</title>
    <category term="technology" scheme="http://example.org/categories"/>
    <summary>Deep dive into zero-cost abstractions.</summary>
    <author>
      <name>Charlie Brown</name>
    </author>
  </entry>
</feed>"#;

#[test]
fn test_find_path_multi_level() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    let author_name = root.find_path(&["author", "name"]);
    assert!(author_name.is_some());
    assert_eq!(author_name.unwrap().borrow().text(), "John Doe");

    let first_entry_author = root.find_path(&["entry", "author", "name"]);
    assert!(first_entry_author.is_some());
    assert_eq!(first_entry_author.unwrap().borrow().text(), "Alice Smith");
}

#[test]
fn test_find_path_missing_and_empty() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    assert!(root.find_path(&[]).is_none());
    assert!(root.find_path(&["nonexistent"]).is_none());
    assert!(root.find_path(&["entry", "nonexistent", "name"]).is_none());
}

#[test]
fn test_find_path_text() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    assert_eq!(
        root.find_path_text(&["title"]),
        Some("Example Feed".to_string())
    );
    assert_eq!(
        root.find_path_text(&["author", "email"]),
        Some("johndoe@example.com".to_string())
    );
    assert_eq!(root.find_path_text(&["author", "phone"]), None);
}

#[test]
fn test_select_by_tag_and_attributes() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    // Select all entries
    let all_entries = root.select("entry");
    assert_eq!(all_entries.len(), 3);

    // Select published entries only
    let published = root.select("entry[status=published]");
    assert_eq!(published.len(), 2);
    assert_eq!(published[0].borrow().find_attrib("id"), Some("e1"));
    assert_eq!(published[1].borrow().find_attrib("id"), Some("e3"));

    // Select draft entries
    let drafts = root.select("entry[status='draft']");
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].borrow().find_attrib("id"), Some("e2"));

    // Select entries with id attribute present
    let with_id = root.select("entry[id]");
    assert_eq!(with_id.len(), 3);
}

#[test]
fn test_select_nested_paths() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    // Select entry titles
    let titles = root.select("entry/title");
    assert_eq!(titles.len(), 3);
    assert_eq!(titles[0].borrow().text(), "First Article");
    assert_eq!(titles[1].borrow().text(), "Draft Notes");
    assert_eq!(titles[2].borrow().text(), "Advanced Rust Systems");

    // Select authors of published entries
    let pub_authors = root.select("entry[status=published]/author/name");
    assert_eq!(pub_authors.len(), 2);
    assert_eq!(pub_authors[0].borrow().text(), "Alice Smith");
    assert_eq!(pub_authors[1].borrow().text(), "Charlie Brown");
}

#[test]
fn test_select_wildcard() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    // All children of author
    let author_fields = root.select("author/*");
    assert_eq!(author_fields.len(), 2);
    let names: Vec<_> = author_fields
        .iter()
        .map(|n| n.borrow().name().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["name", "email"]);
}

#[test]
fn test_select_first_helper() {
    let doc = DomParser::parse_str(ATOM_FEED).expect("parse feed");
    let root = doc.borrow();

    let first_pub = root.select_first("entry[status=published]");
    assert!(first_pub.is_some());
    assert_eq!(first_pub.unwrap().borrow().find_attrib("id"), Some("e1"));

    let none = root.select_first("entry[status=archived]");
    assert!(none.is_none());
}
