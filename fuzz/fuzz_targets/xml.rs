#![no_main]

use iksemel::{DomParser, Parser, ParserLimits, Result, SaxHandler, TagType};
use libfuzzer_sys::fuzz_target;

struct Sink;
impl SaxHandler for Sink {
    fn on_tag(&mut self, _: &str, _: &[(String, String)], _: TagType) -> Result<()> {
        Ok(())
    }
    fn on_cdata(&mut self, _: &str) -> Result<()> {
        Ok(())
    }
}

fn limits() -> ParserLimits {
    ParserLimits {
        max_depth: 32,
        max_entity_expansions: 256,
        max_attributes: 64,
        max_token_size: 4096,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 65536 {
        return;
    }
    let Ok(xml) = std::str::from_utf8(data) else {
        return;
    };
    let _ = DomParser::parse_str_with_limits(xml, limits());
    let mut parser = Parser::with_limits(Sink, limits());
    // Split at UTF-8 boundaries and vary chunk size with the input.
    let width = data.first().copied().unwrap_or(0) as usize + 1;
    let mut start = 0;
    for (end, _) in xml.char_indices() {
        if end - start >= width {
            if parser.parse(&xml[start..end]).is_err() {
                return;
            }
            start = end;
        }
    }
    if parser.parse(&xml[start..]).is_ok() {
        let _ = parser.parse("");
    }
});
