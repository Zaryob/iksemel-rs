use clap::Parser;
use iksemel::{IksError, Parser as IksParser, Result, SaxHandler, TagType};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read};

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
    tag_counts: HashMap<String, u32>,
}

impl SaxHandler for TagHandler {
    fn on_tag(&mut self, name: &str, _attrs: &[(String, String)], tag_type: TagType) -> Result<()> {
        match tag_type {
            TagType::Open => {
                self.tag_stack.push(name.to_string());
                self.stats.level += 1;
                if self.stats.level > self.stats.max_depth {
                    self.stats.max_depth = self.stats.level;
                }
            }
            TagType::Close => {
                if let Some(expected) = self.tag_stack.pop() {
                    if expected != name {
                        return Err(IksError::BadXml);
                    }
                } else {
                    return Err(IksError::BadXml);
                }
                self.stats.level -= 1;
                self.stats.nr_tags += 1;
                *self.tag_counts.entry(name.to_string()).or_insert(0) += 1;
            }
            TagType::Single => {
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
    let handler = TagHandler {
        stats: Stats::default(),
        tag_stack: Vec::new(),
        tag_counts: HashMap::new(),
    };

    let mut parser = IksParser::new(handler);
    let mut reader: Box<dyn Read> = match file_path {
        Some(path) => Box::new(BufReader::new(File::open(path)?)),
        None => Box::new(io::stdin()),
    };

    let mut raw_buf = vec![0u8; 4096];
    let mut leftover = Vec::new();

    loop {
        let n = reader.read(&mut raw_buf)?;
        if n == 0 {
            break;
        }

        let mut data = std::mem::take(&mut leftover);
        data.extend_from_slice(&raw_buf[..n]);

        match std::str::from_utf8(&data) {
            Ok(valid_str) => {
                parser.parse(valid_str)?;
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    let valid_str = std::str::from_utf8(&data[..valid_up_to]).unwrap();
                    parser.parse(valid_str)?;
                }
                leftover.extend_from_slice(&data[valid_up_to..]);
            }
        }
    }

    if !leftover.is_empty() {
        return Err(IksError::BadXml);
    }

    // Flush parser
    parser.parse("")?;

    let handler = parser.handler();
    if !handler.tag_stack.is_empty() {
        return Err(IksError::BadXml);
    }

    if let Some(path) = file_path {
        println!("File '{}':", path);
    }

    if args.stats {
        println!(
            "Tags: {} pairs, {} single, {} max depth.",
            handler.stats.nr_tags, handler.stats.nr_stags, handler.stats.max_depth
        );
        println!(
            "Total size of character data: {} bytes.",
            handler.stats.cdata_size
        );
    }

    if args.histogram {
        println!("\nHistogram of {} unique tags:", handler.tag_counts.len());
        let mut sorted: Vec<_> = handler.tag_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (tag, count) in sorted {
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
