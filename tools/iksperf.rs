use clap::{Parser, ValueEnum};
use iksemel::{
    build_muc_join, build_ping, sha1_hex, DataForm, DataFormType, DomParser, FormField,
    Parser as IksParser, Result, SaxHandler, TagType, XmlWriter,
};
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(author, version, about = "iksemel XML & XMPP benchmark suite", long_about = None)]
struct Args {
    /// Input file path (if omitted, synthetic XML payload will be generated)
    #[arg(short, long)]
    input: Option<String>,

    /// Synthetic payload size in KB if no input file is specified
    #[arg(long, default_value = "1024")]
    synthetic_kb: usize,

    /// Number of benchmark iterations
    #[arg(short = 'n', long = "iterations", default_value = "5")]
    iterations: usize,

    /// Block size for chunked parsing (bytes)
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
    Writer,
    Serialize,
    Query,
    Xep,
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
    fn on_tag(
        &mut self,
        _name: &str,
        _attributes: &[(String, String)],
        _tag_type: TagType,
    ) -> Result<()> {
        self.tag_count += 1;
        Ok(())
    }

    fn on_cdata(&mut self, data: &str) -> Result<()> {
        self.cdata_bytes += data.len();
        Ok(())
    }
}

fn generate_synthetic_xml(target_bytes: usize) -> String {
    let mut out = String::with_capacity(target_bytes + 256);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<benchmark_suite xmlns=\"urn:iksemel:perf\">\n");

    let mut id = 1usize;
    while out.len() < target_bytes {
        use std::fmt::Write;
        let _ = write!(
            out,
            "  <record id=\"rec_{}\" type=\"metric\" timestamp=\"2025-07-30T15:00:00Z\">\n    <source host=\"node-{}.cluster.local\" dc=\"eu-west-1\"/>\n    <payload encoding=\"plain\">Sample telemetry data payload for index {} &amp; testing entities &lt;&gt;</payload>\n    <metrics cpu=\"{:.2}\" mem=\"{}\" load=\"{:.1}\"/>\n  </record>\n",
            id,
            id % 16,
            id,
            (id % 100) as f64 * 0.95,
            (id * 1024) % 65536,
            (id % 10) as f64 * 0.4
        );
        id += 1;
    }

    out.push_str("</benchmark_suite>\n");
    out
}

fn sax_test(text: &str, chunk_size: usize) -> Result<usize> {
    let handler = TestHandler::new();
    let mut parser = IksParser::new(handler);

    let mut pos = 0;
    while pos < text.len() {
        let end = (pos + chunk_size).min(text.len());
        parser.parse(&text[pos..end])?;
        pos = end;
    }
    parser.parse("")?;
    Ok(parser.handler().tag_count)
}

fn dom_test(text: &str, chunk_size: usize) -> Result<()> {
    let parser = DomParser::new()?;
    let mut sax_parser = IksParser::new(parser);

    let mut pos = 0;
    while pos < text.len() {
        let end = (pos + chunk_size).min(text.len());
        sax_parser.parse(&text[pos..end])?;
        pos = end;
    }
    sax_parser.parse("")?;
    Ok(())
}

fn serialize_test(dom: &iksemel::IksNode) -> usize {
    let serialized = dom.to_string();
    serialized.len()
}

fn writer_test(dom: &iksemel::IksNode) -> Result<usize> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut writer = XmlWriter::new(&mut buf);
    writer.write_node(dom)?;
    Ok(buf.len())
}

fn query_test(dom: &iksemel::IksNode) -> usize {
    let mut matches = 0;
    let records = dom.select("record[type=metric]");
    matches += records.len();
    for rec in &records {
        let rec_ref = rec.borrow();
        if let Some(payload) = rec_ref.find_path_text(&["payload"]) {
            matches += payload.len();
        }
    }
    matches
}

fn xep_throughput_test(count: usize) -> Result<usize> {
    let mut total_bytes = 0;
    for i in 0..count {
        let ping = build_ping(&format!("ping_{}", i), Some("server.example.org"));
        total_bytes += ping.to_string().len();

        let mut form = DataForm::new(DataFormType::Form);
        form.add_field(FormField::new("user").with_value("alice"));
        let form_node = form.to_node();
        total_bytes += form_node.to_string().len();

        let muc = build_muc_join("chat@muc.example.org/bot", Some("pass"), Some(100));
        total_bytes += muc.to_string().len();
    }
    Ok(total_bytes)
}

fn throughput_mb_s(bytes: usize, duration: Duration) -> f64 {
    let secs = duration.as_secs_f64();
    if secs <= 0.0 {
        0.0
    } else {
        (bytes as f64 / (1024.0 * 1024.0)) / secs
    }
}

fn print_benchmark_row(
    label: &str,
    best_duration: Duration,
    avg_duration: Duration,
    best_mb_s: f64,
    avg_mb_s: f64,
) {
    println!(
        "  {:<26} | Best: {:>9.2?} ({:>7.2} MB/s) | Avg: {:>9.2?} ({:>7.2} MB/s)",
        label, best_duration, best_mb_s, avg_duration, avg_mb_s
    );
}

fn benchmark_step<F>(name: &str, iterations: usize, bytes: usize, mut op: F)
where
    F: FnMut() -> Result<()>,
{
    let mut total_duration = Duration::ZERO;
    let mut best_duration = Duration::MAX;

    for _ in 0..iterations {
        let start = Instant::now();
        if let Err(e) = op() {
            eprintln!("Error in {}: {:?}", name, e);
            return;
        }
        let elapsed = start.elapsed();
        total_duration += elapsed;
        if elapsed < best_duration {
            best_duration = elapsed;
        }
    }

    let avg_duration = total_duration / (iterations as u32);
    let best_mb_s = throughput_mb_s(bytes, best_duration);
    let avg_mb_s = throughput_mb_s(bytes, avg_duration);

    print_benchmark_row(name, best_duration, avg_duration, best_mb_s, avg_mb_s);
}

fn main() -> Result<()> {
    let args = Args::parse();

    let xml_data: String = match args.input {
        Some(ref path) => {
            let mut file = File::open(path)?;
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            buf
        }
        None => {
            println!(
                "No input file specified. Generating {} KB synthetic XML benchmark fixture...",
                args.synthetic_kb
            );
            generate_synthetic_xml(args.synthetic_kb * 1024)
        }
    };

    let total_bytes = xml_data.len();
    let size_mb = total_bytes as f64 / (1024.0 * 1024.0);

    println!("================================================================================");
    println!(" iksemel-rs High-Performance XML Benchmark Suite");
    println!(
        " Payload Size: {:.2} MB ({} bytes) | Iterations: {} | Chunk Size: {} bytes",
        size_mb, total_bytes, args.iterations, args.block_size
    );
    println!("================================================================================");

    if args.test == TestType::All || args.test == TestType::Sax {
        benchmark_step(
            "SAX Parser (Streaming)",
            args.iterations,
            total_bytes,
            || {
                sax_test(&xml_data, args.block_size)?;
                Ok(())
            },
        );
    }

    if args.test == TestType::All || args.test == TestType::Dom {
        benchmark_step(
            "DOM Parser (Tree Build)",
            args.iterations,
            total_bytes,
            || {
                dom_test(&xml_data, args.block_size)?;
                Ok(())
            },
        );
    }

    if args.test == TestType::All
        || args.test == TestType::Writer
        || args.test == TestType::Serialize
    {
        // Pre-parse DOM once for serialization benchmarks
        let dom_root = DomParser::parse_str(&xml_data)?;
        let dom_ref = dom_root.borrow();

        if args.test == TestType::All || args.test == TestType::Writer {
            benchmark_step(
                "XmlWriter (Stream Buffer)",
                args.iterations,
                total_bytes,
                || {
                    writer_test(&dom_ref)?;
                    Ok(())
                },
            );
        }

        if args.test == TestType::All || args.test == TestType::Serialize {
            benchmark_step(
                "DOM to_string() (Alloc)",
                args.iterations,
                total_bytes,
                || {
                    let _ = serialize_test(&dom_ref);
                    Ok(())
                },
            );
        }

        if args.test == TestType::All || args.test == TestType::Query {
            benchmark_step("DOM Path & Selectors", args.iterations, total_bytes, || {
                let _ = query_test(&dom_ref);
                Ok(())
            });
        }
    }

    if args.test == TestType::All || args.test == TestType::Xep {
        let xep_samples = 2000usize;
        benchmark_step(
            "XEP Stanza Build/Gen (2k)",
            args.iterations,
            xep_samples * 250,
            || {
                let _ = xep_throughput_test(xep_samples)?;
                Ok(())
            },
        );
    }

    if args.test == TestType::All || args.test == TestType::Sha1 {
        let bytes_slice = xml_data.as_bytes();
        benchmark_step("SHA-1 Digest", args.iterations, total_bytes, || {
            let _ = sha1_hex(bytes_slice);
            Ok(())
        });
    }

    println!("================================================================================");

    Ok(())
}
