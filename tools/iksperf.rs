use std::fs::File;
use std::io::{self, Read};
use std::time::Instant;
use clap::{Parser, ValueEnum};
use iksemel::{Parser as IksParser, SaxHandler, Result, DomParser, IksNode};
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file path
    #[arg(short, long)]
    input: String,
    
    /// Block size for chunked parsing
    #[arg(short, long, default_value = "4096")]
    block_size: usize,
}

#[derive(ValueEnum, Clone, Debug)]
enum TestType {
    All,
    Sax,
    Dom,
    Serialize,
    Sha1,
}

struct TestHandler {
    tags: Vec<String>,
    cdata: Vec<String>,
}

impl TestHandler {
    fn new() -> Self {
        TestHandler {
            tags: Vec::new(),
            cdata: Vec::new(),
        }
    }
}

impl SaxHandler for TestHandler {
    fn on_tag(&mut self, name: &str, _attributes: &[(String, String)], _tag_type: iksemel::TagType) -> Result<()> {
        self.tags.push(name.to_string());
        Ok(())
    }
    
    fn on_cdata(&mut self, data: &str) -> Result<()> {
        self.cdata.push(data.to_string());
        Ok(())
    }
}

fn sax_test(data: &[u8], chunk_size: usize) -> Result<()> {
    let handler = TestHandler::new();
    let mut parser = IksParser::new(handler);
    
    let mut pos = 0;
    while pos < data.len() {
        let chunk_size = chunk_size.min(data.len() - pos);
        let chunk = String::from_utf8_lossy(&data[pos..pos + chunk_size]);
        parser.parse(&chunk)?;
        pos += chunk_size;
    }
    parser.parse("")?;
    Ok(())
}

fn dom_test(data: &[u8], chunk_size: usize) -> Result<()> {
    let parser = DomParser::new()?;
    let mut sax_parser = IksParser::new(parser);
    
    let mut pos = 0;
    while pos < data.len() {
        let chunk_size = chunk_size.min(data.len() - pos);
        let chunk = String::from_utf8_lossy(&data[pos..pos + chunk_size]);
        sax_parser.parse(&chunk)?;
        pos += chunk_size;
    }
    sax_parser.parse("")?;
    Ok(())
}

fn serialize_test(data: &[u8]) -> Result<()> {
    let parser = DomParser::new()?;
    let mut sax_parser = IksParser::new(parser);
    
    let mut pos = 0;
    while pos < data.len() {
        let chunk_size = (data.len() - pos).min(4096);
        let chunk = String::from_utf8_lossy(&data[pos..pos + chunk_size]);
        sax_parser.parse(&chunk)?;
        pos += chunk_size;
    }
    sax_parser.parse("")?;
    Ok(())
}

fn sha1_test(data: &[u8]) -> Result<()> {
    use sha1::{Sha1, Digest};
    
    let start = Instant::now();
    let mut hasher = Sha1::new();
    hasher.update(data);
    let result = hasher.finalize();
    let duration = start.elapsed();
    
    println!("SHA1: hashing took {:?}", duration);
    println!("SHA1: hash [{}]", hex::encode(result));
    
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let mut file = std::fs::File::open(&args.input)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    
    println!("Running performance tests on {} bytes...", data.len());
    
    // SAX parsing test
    let start = Instant::now();
    sax_test(&data, args.block_size)?;
    let duration = start.elapsed();
    println!("SAX parsing: {:?}", duration);
    
    // DOM parsing test
    let start = Instant::now();
    dom_test(&data, args.block_size)?;
    let duration = start.elapsed();
    println!("DOM parsing: {:?}", duration);
    
    // Serialization test
    let start = Instant::now();
    serialize_test(&data)?;
    let duration = start.elapsed();
    println!("Serialization: {:?}", duration);
    
    Ok(())
} 