use std::env;
use std::fs::File;
use std::io::{self, Read, BufReader};
use std::path::Path;
use clap::{Parser, ValueEnum};
use iksemel::{Parser as IksParser, SaxHandler, IksError, Result};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input XML file (or stdin if not specified)
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Print statistics
    #[arg(short = 's', long = "stats")]
    stats: bool,

    /// Print tag histogram
    #[arg(short = 't', long = "histogram")]
    histogram: bool,
}

#[derive(Default)]
struct Stats {
    level: u32,
    max_depth: u32,
    nr_tags: u32,
    nr_stags: u32,
    cdata_size: usize,
}

struct TagHandler {
    stats: Stats,
    tag_stack: Vec<String>,
    tag_counts: std::collections::HashMap<String, u32>,
}

impl SaxHandler for TagHandler {
    fn on_tag(&mut self, name: &str, _attrs: &[(String, String)], tag_type: iksemel::TagType) -> Result<()> {
        match tag_type {
            iksemel::TagType::Open => {
                self.tag_stack.push(name.to_string());
                self.stats.level += 1;
                if self.stats.level > self.stats.max_depth {
                    self.stats.max_depth = self.stats.level;
                }
            }
            iksemel::TagType::Close => {
                if let Some(expected) = self.tag_stack.pop() {
                    if expected != name {
                        return Err(IksError::BadXml);
                    }
                }
                self.stats.level -= 1;
                self.stats.nr_tags += 1;
                *self.tag_counts.entry(name.to_string()).or_insert(0) += 1;
            }
            iksemel::TagType::Single => {
                self.stats.nr_stags += 1;
                *self.tag_counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
        Ok(())
    }

    fn on_cdata(&mut self, data: &str) -> Result<()> {
        self.stats.cdata_size += data.len();
        Ok(())
    }
}

fn check_file(file_path: Option<&str>, args: &Args) -> Result<()> {
    let mut handler = TagHandler {
        stats: Stats::default(),
        tag_stack: Vec::new(),
        tag_counts: std::collections::HashMap::new(),
    };

    let mut parser = IksParser::new(handler);
    let mut reader: Box<dyn Read> = match file_path {
        Some(path) => Box::new(BufReader::new(File::open(path)?)),
        None => Box::new(io::stdin()),
    };

    let mut buffer = vec![0; 4096];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buffer[..n]);
        parser.parse(&chunk)?;
    }
    parser.parse("")?;

    if let Some(path) = file_path {
        println!("File '{}':", path);
    }

    let handler = parser.handler();
    if args.stats {
        println!("Tags: {} pairs, {} single, {} max depth.",
            handler.stats.nr_tags,
            handler.stats.nr_stags,
            handler.stats.max_depth
        );
        println!("Total size of character data: {} bytes.",
            handler.stats.cdata_size
        );
    }

    if args.histogram {
        println!("\nHistogram of {} unique tags:", handler.tag_counts.len());
        for (tag, count) in handler.tag_counts.iter() {
            println!("<{}> {} times.", tag, count);
        }
    }

    Ok(())
}

fn main() {
    let args = Args::parse();

    if let Err(e) = check_file(args.file.as_deref(), &args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
} 