use iksemel::{
    DataForm, DataFormType, FieldOption, FieldType, FormField, IksNode, XMLNS_DATA_FORMS,
};

#[test]
fn test_data_form_build_and_parse_roundtrip() {
    let mut form = DataForm::new(DataFormType::Form)
        .with_title("Bot Configuration")
        .with_instruction("Fill out the following configuration fields.")
        .with_instruction("Leave blank for defaults.");

    let field_name = FormField::new("botname")
        .with_type(FieldType::TextSingle)
        .with_label("Bot Name")
        .with_desc("Display name for the automated assistant")
        .with_required(true)
        .with_value("EchoBot");

    let field_password = FormField::new("password")
        .with_type(FieldType::TextPrivate)
        .with_label("Admin Password")
        .with_value("supersecret");

    let field_enabled = FormField::new("enabled")
        .with_type(FieldType::Boolean)
        .with_label("Enable on Start")
        .with_value("1");

    let field_channels = FormField::new("channels")
        .with_type(FieldType::ListMulti)
        .with_label("Active Channels")
        .with_option("general", Some("General Discussion"))
        .with_option("dev", Some("Developers"))
        .with_option("ops", None::<&str>)
        .with_values(vec!["general", "dev"]);

    form.add_field(field_name);
    form.add_field(field_password);
    form.add_field(field_enabled);
    form.add_field(field_channels);

    let node = form.to_node();
    assert_eq!(node.name(), Some("x"));
    assert_eq!(node.find_attrib("xmlns"), Some(XMLNS_DATA_FORMS));
    assert_eq!(node.find_attrib("type"), Some("form"));

    // Parse back
    let parsed = DataForm::from_node(&node).expect("Failed to parse DataForm");
    assert_eq!(parsed.form_type, Some(DataFormType::Form));
    assert_eq!(parsed.title.as_deref(), Some("Bot Configuration"));
    assert_eq!(parsed.instructions.len(), 2);
    assert_eq!(
        parsed.instructions[0],
        "Fill out the following configuration fields."
    );

    // Verify fields
    let botname = parsed.get_field("botname").expect("botname field missing");
    assert_eq!(botname.field_type, Some(FieldType::TextSingle));
    assert_eq!(botname.label.as_deref(), Some("Bot Name"));
    assert_eq!(
        botname.desc.as_deref(),
        Some("Display name for the automated assistant")
    );
    assert!(botname.required);
    assert_eq!(botname.first_value(), Some("EchoBot"));

    let enabled = parsed.get_field("enabled").expect("enabled field missing");
    assert_eq!(enabled.as_boolean(), Some(true));

    let channels = parsed
        .get_field("channels")
        .expect("channels field missing");
    assert_eq!(channels.values, vec!["general", "dev"]);
    assert_eq!(channels.options.len(), 3);
    assert_eq!(
        channels.options[0],
        FieldOption {
            label: Some("General Discussion".to_string()),
            value: "general".to_string(),
        }
    );
    assert_eq!(
        channels.options[2],
        FieldOption {
            label: None,
            value: "ops".to_string(),
        }
    );
}

#[test]
fn test_data_form_submit_and_cancel() {
    let mut submit_form = DataForm::new(DataFormType::Submit);
    submit_form.add_field(FormField::new("username").with_value("alice"));
    submit_form.add_field(FormField::new("role").with_value("admin"));

    let node = submit_form.to_node();
    let parsed_submit = DataForm::from_node(&node).unwrap();
    assert_eq!(parsed_submit.form_type, Some(DataFormType::Submit));
    assert_eq!(parsed_submit.get_value("username"), Some("alice"));
    assert_eq!(parsed_submit.get_value("role"), Some("admin"));

    let cancel_form = DataForm::new(DataFormType::Cancel);
    let cancel_node = cancel_form.to_node();
    let parsed_cancel = DataForm::from_node(&cancel_node).unwrap();
    assert_eq!(parsed_cancel.form_type, Some(DataFormType::Cancel));
}

#[test]
fn test_data_form_reported_and_items() {
    let mut result_form = DataForm::new(DataFormType::Result);

    // Reported columns definition
    result_form.reported.push(
        FormField::new("user_jid")
            .with_type(FieldType::JidSingle)
            .with_label("Jabber ID"),
    );
    result_form.reported.push(
        FormField::new("status")
            .with_type(FieldType::TextSingle)
            .with_label("Online Status"),
    );

    // Item 1
    result_form.items.push(vec![
        FormField::new("user_jid").with_value("alice@example.com"),
        FormField::new("status").with_value("Available"),
    ]);

    // Item 2
    result_form.items.push(vec![
        FormField::new("user_jid").with_value("bob@example.com"),
        FormField::new("status").with_value("Busy"),
    ]);

    let node = result_form.to_node();
    let parsed = DataForm::from_node(&node).unwrap();

    assert_eq!(parsed.form_type, Some(DataFormType::Result));
    assert_eq!(parsed.reported.len(), 2);
    assert_eq!(parsed.reported[0].var, "user_jid");
    assert_eq!(parsed.items.len(), 2);
    assert_eq!(parsed.items[0][0].first_value(), Some("alice@example.com"));
    assert_eq!(parsed.items[1][1].first_value(), Some("Busy"));
}

#[test]
fn test_data_form_attachment_and_extraction() {
    let mut message = IksNode::new_tag("message");
    message.add_attribute("to", "bob@example.com");
    message.add_attribute("from", "alice@example.com");

    let mut form = DataForm::new(DataFormType::Form).with_title("Poll");
    form.add_field(
        FormField::new("vote")
            .with_type(FieldType::ListSingle)
            .with_option("yes", Some("Yes"))
            .with_option("no", Some("No")),
    );

    form.attach_to(&mut message);

    let extracted = DataForm::extract_from(&message).expect("Expected form to be extracted");
    assert_eq!(extracted.title.as_deref(), Some("Poll"));
    let vote_field = extracted.get_field("vote").unwrap();
    assert_eq!(vote_field.options.len(), 2);
}

#[test]
fn test_data_form_invalid_xml() {
    let wrong_tag = IksNode::new_tag("not-x");
    assert!(DataForm::from_node(&wrong_tag).is_err());

    let mut wrong_ns = IksNode::new_tag("x");
    wrong_ns.add_attribute("xmlns", "urn:wrong:namespace");
    assert!(DataForm::from_node(&wrong_ns).is_err());
}
