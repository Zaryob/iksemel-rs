use iksemel::{DomParser, Parser as IksParser, Result, SaxHandler, TagType, XmlWriter};
use std::hint::black_box;
use std::io::Cursor;
use std::time::Instant;

fn generate_synthetic_xml(size_kb: usize) -> String {
    let mut s = String::with_capacity(size_kb * 1024 + 256);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<stream:stream xmlns:stream=\"http://etherx.jabber.org/streams\" xmlns=\"jabber:client\" to=\"example.com\" version=\"1.0\">\n");
    let target_bytes = size_kb * 1024;
    let mut i = 0usize;
    while s.len() < target_bytes {
        i += 1;
        s.push_str(&format!(
            "  <message id=\"msg_{}\" to=\"user{}@example.com/res\" type=\"chat\">\n    <body priority=\"normal\">Hello world &amp; welcome &lt;user{}&gt;!</body>\n    <active xmlns=\"http://jabber.org/protocol/chatstates\"/>\n  </message>\n",
            i, i % 100, i
        ));
    }
    s.push_str("</stream:stream>\n");
    s
}

struct NullHandler {
    tag_count: usize,
}

impl SaxHandler for NullHandler {
    fn on_tag(
        &mut self,
        _name: &str,
        _attributes: &[(String, String)],
        _tag_type: TagType,
    ) -> Result<()> {
        self.tag_count += 1;
        Ok(())
    }

    fn on_cdata(&mut self, _data: &str) -> Result<()> {
        Ok(())
    }
}

fn main() {
    let is_debug = cfg!(debug_assertions);
    let size_kb = if is_debug { 64 } else { 1024 };
    let iterations = if is_debug { 1 } else { 5 };

    let xml = generate_synthetic_xml(size_kb);
    let bytes = xml.as_bytes();
    let mb = (bytes.len() as f64) / (1024.0 * 1024.0);

    println!(
        "Payload bytes: {}; throughput unit: MiB/s; best of {} samples",
        bytes.len(),
        iterations
    );
    println!("Operations have different semantics; these are not equivalent-work speed ratios.");
    println!("================================================================================");
    println!(
        " Rust XML Libraries Comparative Benchmark (Payload: {:.2} MB, Iterations: {})",
        mb, iterations
    );
    println!("================================================================================");
    println!(
        "  {:<32} | {:>10} | {:>10} | {:>12}",
        "Operation / Library", "Best Time", "Avg Time", "Throughput"
    );
    println!("  {:-<32}-+-{:-<10}-+-{:-<10}-+-{:-<12}", "", "", "", "");

    // 1. SAX / Streaming / Pull Parsing
    // 1.1 iksemel SAX
    let mut iks_sax_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let handler = NullHandler { tag_count: 0 };
        let mut parser = IksParser::new(handler);
        for chunk in bytes.chunks(4096) {
            parser
                .parse(std::str::from_utf8(chunk).unwrap())
                .expect("synthetic SAX input must parse");
        }
        parser.parse("").expect("synthetic SAX flush must succeed");
        assert!(black_box(parser.handler().tag_count) > 0);
        iks_sax_times.push(start.elapsed());
    }
    let iks_sax_best = *iks_sax_times.iter().min().unwrap();
    let iks_sax_avg = iks_sax_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let iks_sax_mb = mb / iks_sax_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "iksemel SAX (Streaming)", iks_sax_best, iks_sax_avg, iks_sax_mb
    );

    // 1.2 quick-xml Reader
    let mut qxml_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let mut reader = quick_xml::Reader::from_str(&xml);
        reader.config_mut().check_end_names = true;
        let mut buf = Vec::with_capacity(1024);
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => buf.clear(),
                Err(e) => panic!("quick-xml error: {:?}", e),
            }
        }
        qxml_times.push(start.elapsed());
    }
    let qxml_best = *qxml_times.iter().min().unwrap();
    let qxml_avg = qxml_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let qxml_mb = mb / qxml_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "quick-xml Reader (Pull)", qxml_best, qxml_avg, qxml_mb
    );

    // 1.3 xml-rs EventReader
    let mut xmlrs_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let reader = xml::EventReader::new(Cursor::new(bytes));
        for event in reader {
            match event {
                Ok(xml::reader::XmlEvent::EndDocument) => break,
                Ok(_) => {}
                Err(e) => panic!("xml-rs error: {:?}", e),
            }
        }
        xmlrs_times.push(start.elapsed());
    }
    let xmlrs_best = *xmlrs_times.iter().min().unwrap();
    let xmlrs_avg = xmlrs_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let xmlrs_mb = mb / xmlrs_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "xml-rs EventReader (Pull)", xmlrs_best, xmlrs_avg, xmlrs_mb
    );

    println!("  {:-<32}-+-{:-<10}-+-{:-<10}-+-{:-<12}", "", "", "", "");

    // 2. DOM / Tree Parsing
    // 2.1 iksemel DomParser (Mutable DOM)
    let mut iks_dom_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let _doc = black_box(DomParser::parse_str(black_box(&xml)).unwrap());
        iks_dom_times.push(start.elapsed());
    }
    let iks_dom_best = *iks_dom_times.iter().min().unwrap();
    let iks_dom_avg = iks_dom_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let iks_dom_mb = mb / iks_dom_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "iksemel DOM (Mutable Tree)", iks_dom_best, iks_dom_avg, iks_dom_mb
    );

    // 2.2 roxmltree Document (Immutable Read-Only Arena)
    let mut rox_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let _doc = black_box(roxmltree::Document::parse(black_box(&xml)).unwrap());
        rox_times.push(start.elapsed());
    }
    let rox_best = *rox_times.iter().min().unwrap();
    let rox_avg = rox_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let rox_mb = mb / rox_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "roxmltree (Read-Only Arena)", rox_best, rox_avg, rox_mb
    );

    println!("  {:-<32}-+-{:-<10}-+-{:-<10}-+-{:-<12}", "", "", "", "");

    // 3. Serialization
    // 3.1 iksemel XmlWriter
    let mut iks_wr_times = Vec::new();
    let dom_tree = DomParser::parse_str(&xml).unwrap();
    for _ in 0..iterations {
        let start = Instant::now();
        let mut out = Vec::with_capacity(bytes.len() + 1024);
        let mut writer = XmlWriter::new(&mut out);
        writer.write_node(&dom_tree.borrow()).unwrap();
        black_box(&out);
        iks_wr_times.push(start.elapsed());
    }
    let iks_wr_best = *iks_wr_times.iter().min().unwrap();
    let iks_wr_avg = iks_wr_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let iks_wr_mb = mb / iks_wr_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "iksemel XmlWriter (Stream)", iks_wr_best, iks_wr_avg, iks_wr_mb
    );

    // 3.2 quick-xml Writer (emitting stanzas from parsed events)
    let mut qxml_wr_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let mut reader = quick_xml::Reader::from_str(&xml);
        let mut out = Cursor::new(Vec::with_capacity(bytes.len() + 1024));
        let mut writer = quick_xml::Writer::new(&mut out);
        let mut buf = Vec::with_capacity(1024);
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(e) => writer.write_event(e).unwrap(),
                Err(e) => panic!("err: {:?}", e),
            }
            buf.clear();
        }
        qxml_wr_times.push(start.elapsed());
        black_box(out.get_ref());
    }
    let qxml_wr_best = *qxml_wr_times.iter().min().unwrap();
    let qxml_wr_avg = qxml_wr_times.iter().sum::<std::time::Duration>() / iterations as u32;
    let qxml_wr_mb = mb / qxml_wr_best.as_secs_f64();
    println!(
        "  {:<32} | {:>9.2?} | {:>9.2?} | {:>9.2} MiB/s",
        "quick-xml Writer (Roundtrip)", qxml_wr_best, qxml_wr_avg, qxml_wr_mb
    );

    println!("================================================================================");
}
