use iksemel::{DomParser, IksError, Parser, ParserLimits, Result, SaxHandler, TagType};

struct NullHandler;

impl SaxHandler for NullHandler {
    fn on_tag(
        &mut self,
        _name: &str,
        _attributes: &[(String, String)],
        _tag_type: TagType,
    ) -> Result<()> {
        Ok(())
    }

    fn on_cdata(&mut self, _data: &str) -> Result<()> {
        Ok(())
    }
}

#[test]
fn test_security_max_depth_exceeded() {
    let limits = ParserLimits {
        max_depth: 8,
        ..Default::default()
    };

    let mut xml = String::new();
    for i in 0..12 {
        xml.push_str(&format!("<level_{}>", i));
    }
    xml.push_str("deep content");
    for i in (0..12).rev() {
        xml.push_str(&format!("</level_{}>", i));
    }

    let mut parser = Parser::with_limits(NullHandler, limits);
    let result = parser.parse(&xml);

    match result {
        Err(IksError::MaxDepthExceeded) => {}
        other => panic!("Expected MaxDepthExceeded error, got: {:?}", other),
    }

    // Also test through DomParser
    let dom_res = DomParser::parse_str_with_limits(&xml, limits);
    match dom_res {
        Err(IksError::MaxDepthExceeded) => {}
        other => panic!(
            "Expected MaxDepthExceeded error on DomParser, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_security_max_depth_within_limit() {
    let limits = ParserLimits {
        max_depth: 10,
        ..Default::default()
    };

    let mut xml = String::new();
    for i in 0..5 {
        xml.push_str(&format!("<level_{}>", i));
    }
    xml.push_str("safe content");
    for i in (0..5).rev() {
        xml.push_str(&format!("</level_{}>", i));
    }

    let mut parser = Parser::with_limits(NullHandler, limits);
    assert!(parser.parse(&xml).is_ok());

    let dom_res = DomParser::parse_str_with_limits(&xml, limits);
    assert!(dom_res.is_ok());
}

#[test]
fn test_security_max_entity_expansions_exceeded() {
    let limits = ParserLimits {
        max_entity_expansions: 3,
        ..Default::default()
    };

    // Text with 5 entities
    let xml = "<root>Entity expansion &amp; &lt; &gt; &apos; &quot;</root>";

    let mut parser = Parser::with_limits(NullHandler, limits);
    let result = parser.parse(xml);

    match result {
        Err(IksError::MaxEntityExpansionsExceeded) => {}
        other => panic!(
            "Expected MaxEntityExpansionsExceeded error, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_security_attribute_entity_expansions_exceeded() {
    let limits = ParserLimits {
        max_entity_expansions: 2,
        ..Default::default()
    };

    // Attribute containing 4 entities
    let xml = "<root message=\"Hello &amp; welcome &lt;friend&gt; &quot;\"/>";

    let mut parser = Parser::with_limits(NullHandler, limits);
    let result = parser.parse(xml);

    match result {
        Err(IksError::MaxEntityExpansionsExceeded) => {}
        other => panic!(
            "Expected MaxEntityExpansionsExceeded on attribute entities, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_security_max_attributes_exceeded() {
    let limits = ParserLimits {
        max_attributes: 4,
        ..Default::default()
    };

    let xml = "<node a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\" />";

    let mut parser = Parser::with_limits(NullHandler, limits);
    let result = parser.parse(xml);

    match result {
        Err(IksError::MaxAttributesExceeded) => {}
        other => panic!("Expected MaxAttributesExceeded error, got: {:?}", other),
    }
}

#[test]
fn test_security_max_token_size_exceeded() {
    let limits = ParserLimits {
        max_token_size: 32,
        ..Default::default()
    };

    // Attribute value longer than 32 bytes
    let xml =
        "<node attr=\"This is a very long attribute value designed to exceed the token limit\"/>";

    let mut parser = Parser::with_limits(NullHandler, limits);
    let result = parser.parse(xml);

    match result {
        Err(IksError::MaxTokenSizeExceeded) => {}
        other => panic!("Expected MaxTokenSizeExceeded error, got: {:?}", other),
    }
}
