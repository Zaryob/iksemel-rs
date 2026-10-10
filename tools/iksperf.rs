use clap::{Parser, ValueEnum};
use iksemel::{
    DataForm, DataFormType, DomParser, FormField, IksNode, MamQuery, Parser as IksParser, Result,
    SaxHandler, ScramClient, ScramHash, TagType, XmlWriter, build_carbons_enable, build_muc_join,
    build_ping, escape, escape_cow, pbkdf2_hmac_sha256, sha1_hex, wrap_carbon_received,
};
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "iksemel-rs High-Performance XML & XMPP Profiling & Benchmark Suite",
    long_about = None
)]
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

    /// Output full benchmark metrics in structured JSON format
    #[arg(long)]
    json: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum TestType {
    All,
    Sax,
    Dom,
    Writer,
    Serialize,
    Query,
    Escape,
    Scram,
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
            "  <record id=\"rec_{}\" type=\"metric\" timestamp=\"2026-02-11T10:00:00Z\">\n    <source host=\"node-{}.cluster.local\" dc=\"eu-west-1\"/>\n    <payload encoding=\"plain\">Sample telemetry data payload for index {} &amp; testing entities &lt;&gt;</payload>\n    <metrics cpu=\"{:.2}\" mem=\"{}\" load=\"{:.1}\"/>\n  </record>\n",
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

fn serialize_test(dom: &IksNode) -> usize {
    let serialized = dom.to_string();
    serialized.len()
}

fn writer_test(dom: &IksNode) -> Result<usize> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut writer = XmlWriter::new(&mut buf);
    writer.write_node(dom)?;
    Ok(buf.len())
}

fn query_test(dom: &IksNode) -> usize {
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

fn escape_cow_test(samples: &[String]) -> usize {
    let mut total_len = 0;
    for s in samples {
        let res = escape_cow(s);
        total_len += res.len();
    }
    total_len
}

fn escape_alloc_test(samples: &[String]) -> usize {
    let mut total_len = 0;
    for s in samples {
        let res = escape(s);
        total_len += res.len();
    }
    total_len
}

fn scram_test(iterations: usize) -> Result<usize> {
    let mut total = 0;
    for i in 0..iterations {
        let nonce = format!("nonce_val_{}", i);
        let mut client = ScramClient::new(
            ScramHash::Sha256,
            format!("user_{}", i),
            "secretpassword".to_string(),
        )
        .with_nonce(nonce.clone());
        let first_msg = client.client_first_message();
        total += first_msg.len();

        let salt = "c2FsdHNhbHQ=";
        let server_first = format!("r={}servernonce,s={},i=4096", nonce, salt);
        let final_msg = client.process_challenge(&server_first)?;
        total += final_msg.len();
    }
    Ok(total)
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

        let carbon_enable = build_carbons_enable(&format!("c_{}", i));
        total_bytes += carbon_enable.to_string().len();

        let inner_msg = IksNode::new_tag("message");
        let carbon_wrapped = wrap_carbon_received(&inner_msg);
        total_bytes += carbon_wrapped.to_string().len();

        let mam = MamQuery::new()
            .with_query_id(format!("q_{}", i))
            .with_jid("user@example.com")
            .with_rsm_max(25)
            .to_iq(&format!("iq_{}", i));
        total_bytes += mam.to_string().len();
    }
    Ok(total_bytes)
}

#[derive(Debug, Clone)]
struct MetricResult {
    name: String,
    bytes: usize,
    best_duration: Duration,
    avg_duration: Duration,
    p50_duration: Duration,
    p95_duration: Duration,
    max_duration: Duration,
    best_mb_s: f64,
    avg_mb_s: f64,
}

fn throughput_mb_s(bytes: usize, duration: Duration) -> f64 {
    let secs = duration.as_secs_f64();
    if secs <= 0.0 {
        0.0
    } else {
        (bytes as f64 / (1024.0 * 1024.0)) / secs
    }
}

fn benchmark_step<F>(name: &str, iterations: usize, bytes: usize, mut op: F) -> MetricResult
where
    F: FnMut() -> Result<()>,
{
    let mut durations = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        if let Err(e) = op() {
            eprintln!("Error in {}: {:?}", name, e);
            break;
        }
        durations.push(start.elapsed());
    }

    durations.sort();

    let best_duration = durations.first().copied().unwrap_or(Duration::ZERO);
    let max_duration = durations.last().copied().unwrap_or(Duration::ZERO);
    let total_duration: Duration = durations.iter().sum();
    let avg_duration = if durations.is_empty() {
        Duration::ZERO
    } else {
        total_duration / (durations.len() as u32)
    };

    let p50_duration = durations[durations.len() / 2];
    let p95_idx = ((durations.len() as f64 * 0.95).round() as usize).min(durations.len() - 1);
    let p95_duration = durations[p95_idx];

    let best_mb_s = throughput_mb_s(bytes, best_duration);
    let avg_mb_s = throughput_mb_s(bytes, avg_duration);

    MetricResult {
        name: name.to_string(),
        bytes,
        best_duration,
        avg_duration,
        p50_duration,
        p95_duration,
        max_duration,
        best_mb_s,
        avg_mb_s,
    }
}

fn print_metric_table(results: &[MetricResult]) {
    println!(
        "  {:<26} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10}",
        "Benchmark Operation", "Best", "Avg", "P50 (Med)", "P95", "Throughput"
    );
    println!(
        "  {:-<26}-+-{:-<10}-+-{:-<10}-+-{:-<10}-+-{:-<10}-+-{:-<10}",
        "", "", "", "", "", ""
    );
    for r in results {
        println!(
            "  {:<26} | {:>10.2?} | {:>10.2?} | {:>10.2?} | {:>10.2?} | {:>7.2} MB/s",
            r.name, r.best_duration, r.avg_duration, r.p50_duration, r.p95_duration, r.avg_mb_s
        );
    }
}

fn print_json_report(results: &[MetricResult], payload_size: usize, iterations: usize) {
    println!("{{");
    println!("  \"suite\": \"iksemel-rs\",");
    println!("  \"payload_bytes\": {},", payload_size);
    println!("  \"iterations\": {},", iterations);
    println!("  \"metrics\": [");
    for (i, r) in results.iter().enumerate() {
        let is_last = i == results.len() - 1;
        println!("    {{");
        println!("      \"name\": \"{}\",", r.name);
        println!("      \"bytes\": {},", r.bytes);
        println!("      \"best_micros\": {},", r.best_duration.as_micros());
        println!("      \"avg_micros\": {},", r.avg_duration.as_micros());
        println!("      \"p50_micros\": {},", r.p50_duration.as_micros());
        println!("      \"p95_micros\": {},", r.p95_duration.as_micros());
        println!("      \"max_micros\": {},", r.max_duration.as_micros());
        println!("      \"best_mb_s\": {:.2},", r.best_mb_s);
        println!("      \"avg_mb_s\": {:.2}", r.avg_mb_s);
        if is_last {
            println!("    }}");
        } else {
            println!("    }},");
        }
    }
    println!("  ]");
    println!("}}");
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
            if !args.json {
                println!(
                    "No input file specified. Generating {} KB synthetic XML benchmark fixture...",
                    args.synthetic_kb
                );
            }
            generate_synthetic_xml(args.synthetic_kb * 1024)
        }
    };

    let total_bytes = xml_data.len();
    let size_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let mut results = Vec::new();

    if !args.json {
        println!(
            "================================================================================"
        );
        println!(" iksemel-rs High-Performance XML & XMPP Profiling Suite");
        println!(
            " Payload Size: {:.2} MB ({} bytes) | Iterations: {} | Chunk Size: {} bytes",
            size_mb, total_bytes, args.iterations, args.block_size
        );
        println!(
            "================================================================================"
        );
    }

    if args.test == TestType::All || args.test == TestType::Sax {
        results.push(benchmark_step(
            "SAX Parser (Streaming)",
            args.iterations,
            total_bytes,
            || {
                sax_test(&xml_data, args.block_size)?;
                Ok(())
            },
        ));
    }

    if args.test == TestType::All || args.test == TestType::Dom {
        results.push(benchmark_step(
            "DOM Parser (Tree Build)",
            args.iterations,
            total_bytes,
            || {
                dom_test(&xml_data, args.block_size)?;
                Ok(())
            },
        ));
    }

    if args.test == TestType::All
        || args.test == TestType::Writer
        || args.test == TestType::Serialize
        || args.test == TestType::Query
    {
        let dom_root = DomParser::parse_str(&xml_data)?;
        let dom_ref = dom_root.borrow();

        if args.test == TestType::All || args.test == TestType::Writer {
            results.push(benchmark_step(
                "XmlWriter (Stream Buffer)",
                args.iterations,
                total_bytes,
                || {
                    writer_test(&dom_ref)?;
                    Ok(())
                },
            ));
        }

        if args.test == TestType::All || args.test == TestType::Serialize {
            results.push(benchmark_step(
                "DOM to_string() (Alloc)",
                args.iterations,
                total_bytes,
                || {
                    let _ = serialize_test(&dom_ref);
                    Ok(())
                },
            ));
        }

        if args.test == TestType::All || args.test == TestType::Query {
            results.push(benchmark_step(
                "DOM Path & Selectors",
                args.iterations,
                total_bytes,
                || {
                    let _ = query_test(&dom_ref);
                    Ok(())
                },
            ));
        }
    }

    if args.test == TestType::All || args.test == TestType::Escape {
        let sample_strings: Vec<String> = (0..5000)
            .map(|i| {
                if i % 5 == 0 {
                    format!("<record priority=\"high\" entity=\"&test;\" id=\"{}\">", i)
                } else {
                    format!("simple_identifier_without_special_chars_{}", i)
                }
            })
            .collect();
        let sample_bytes: usize = sample_strings.iter().map(|s| s.len()).sum();

        results.push(benchmark_step(
            "Zero-Alloc escape_cow()",
            args.iterations,
            sample_bytes,
            || {
                let _ = escape_cow_test(&sample_strings);
                Ok(())
            },
        ));

        results.push(benchmark_step(
            "Standard escape() (Alloc)",
            args.iterations,
            sample_bytes,
            || {
                let _ = escape_alloc_test(&sample_strings);
                Ok(())
            },
        ));
    }

    if args.test == TestType::All || args.test == TestType::Scram {
        let scram_iterations = 200usize;
        results.push(benchmark_step(
            "SCRAM-SHA-256 Handshake",
            args.iterations,
            scram_iterations * 512,
            || {
                let _ = scram_test(scram_iterations)?;
                Ok(())
            },
        ));

        results.push(benchmark_step(
            "PBKDF2-SHA-256 (4096 iter)",
            args.iterations,
            100 * 32,
            || {
                let mut key = [0u8; 32];
                for _ in 0..100 {
                    pbkdf2_hmac_sha256(b"password", b"saltsalt", 4096, &mut key);
                }
                Ok(())
            },
        ));
    }

    if args.test == TestType::All || args.test == TestType::Xep {
        let xep_samples = 2000usize;
        results.push(benchmark_step(
            "XEP Stanza Build/Gen (2k)",
            args.iterations,
            xep_samples * 350,
            || {
                let _ = xep_throughput_test(xep_samples)?;
                Ok(())
            },
        ));
    }

    if args.test == TestType::All || args.test == TestType::Sha1 {
        let bytes_slice = xml_data.as_bytes();
        results.push(benchmark_step(
            "SHA-1 Digest",
            args.iterations,
            total_bytes,
            || {
                let _ = sha1_hex(bytes_slice);
                Ok(())
            },
        ));
    }

    if args.json {
        print_json_report(&results, total_bytes, args.iterations);
    } else {
        print_metric_table(&results);
        println!(
            "================================================================================"
        );
    }

    Ok(())
}
