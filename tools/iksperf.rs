use std::fs::File;
use std::io::Read;
use std::time::Instant;
use clap::{Parser, ValueEnum};
use iksemel::{sha1_hex, DomParser, Parser as IksParser, Result, SaxHandler, TagType};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file path
    #[arg(short, long)]
    input: String,

    /// Block size for chunked parsing
    #[arg(short, long, default_value = "4096")]
    block_size: usize,

    /// Test type to run
    #[arg(short = 't', long = "test", value_enum, default_value = "all")]
    test: TestType,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum TestType {
    All,
    Sax,
    Dom,
    Serialize,
    Sha1,
}

struct TestHandler {
    tag_count: usize,
    cdata_bytes: usize,
}

impl TestHandler {
    fn new() -> Self {
        TestHandler {
            tag_count: 0,
            cdata_bytes: 0,
        }
    }
}

impl SaxHandler for TestHandler {
    fn on_tag(&mut self, _name: &str, _attributes: &[(String, String)], _tag_type: TagType) -> Result<()> {
        self.tag_count += 1;
        Ok(())
    }

    fn on_cdata(&mut self, data: &str) -> Result<()> {
        self.cdata_bytes += data.len();
        Ok(())
    }
}

fn sax_test(data: &[u8], chunk_size: usize) -> Result<()> {
    let handler = TestHandler::new();
    let mut parser = IksParser::new(handler);

    let text = std::str::from_utf8(data).map_err(|_| iksemel::IksError::BadXml)?;
    let mut pos = 0;
    while pos < text.len() {
        let end = (pos + chunk_size).min(text.len());
        parser.parse(&text[pos..end])?;
        pos = end;
    }
    parser.parse("")?;
    Ok(())
}

fn dom_test(data: &[u8], chunk_size: usize) -> Result<()> {
    let parser = DomParser::new()?;
    let mut sax_parser = IksParser::new(parser);

    let text = std::str::from_utf8(data).map_err(|_| iksemel::IksError::BadXml)?;
    let mut pos = 0;
    while pos < text.len() {
        let end = (pos + chunk_size).min(text.len());
        sax_parser.parse(&text[pos..end])?;
        pos = end;
    }
    sax_parser.parse("")?;
    Ok(())
}

fn serialize_test(data: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(data).map_err(|_| iksemel::IksError::BadXml)?;
    let dom = DomParser::parse_str(text)?;
    let serialized = dom.borrow().to_string();
    let _ = serialized.len();
    Ok(())
}

fn sha1_test(data: &[u8]) {
    let start = Instant::now();
    let hash = sha1_hex(data);
    let duration = start.elapsed();
    println!("SHA1: hashing took {:?}", duration);
    println!("SHA1: hash [{}]", hash);
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut file = File::open(&args.input)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    println!("Running performance tests on {} bytes...", data.len());

    if args.test == TestType::All || args.test == TestType::Sax {
        let start = Instant::now();
        sax_test(&data, args.block_size)?;
        let duration = start.elapsed();
        println!("SAX parsing: {:?}", duration);
    }

    if args.test == TestType::All || args.test == TestType::Dom {
        let start = Instant::now();
        dom_test(&data, args.block_size)?;
        let duration = start.elapsed();
        println!("DOM parsing: {:?}", duration);
    }

    if args.test == TestType::All || args.test == TestType::Serialize {
        let start = Instant::now();
        serialize_test(&data)?;
        let duration = start.elapsed();
        println!("Serialization: {:?}", duration);
    }

    if args.test == TestType::All || args.test == TestType::Sha1 {
        sha1_test(&data);
    }

    Ok(())
}