//! Probe: exercise the import path headlessly and print every returned
//! event. Usage:
//!   cargo run -p reeda-core --example import_probe -- <file>
//!   cargo run -p reeda-core --example import_probe -- --emit-sample <path>

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--emit-sample") {
        let path = args.get(2).expect("usage: import_probe --emit-sample <path>");
        std::fs::write(path, build_sample_epub()).expect("write sample");
        println!("wrote {}", path);
        return;
    }
    let path = args.get(1).unwrap_or_else(|| panic!("usage: import_probe <file>"));
    let data = std::fs::read(&path).expect("read file");

    let root = PathBuf::from("probe_data");
    let _ = std::fs::remove_dir_all(&root);
    let store = reeda_core::BookStore::new(&root).expect("store");
    let db = reeda_core::Database::open(store.root().join("reeda.sqlite")).expect("db");
    let mut app = reeda_core::App::with_store_db(store, db);
    match reeda_core::search::SearchService::open(&root) {
        Ok(s) => app.set_search(s),
        Err(e) => eprintln!("search open failed: {e}"),
    }

    let events = app.import_from_bytes(data, path.clone());
    for ev in &events {
        println!("EVENT: {ev:?}");
    }
    println!("library size: {}", app.snapshot().library.len());
    println!(
        "store contents: {:?}",
        std::fs::read_dir("probe_data/books")
            .map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>())
            .unwrap_or_default()
    );
}

/// Same builder as examples/engine_demo.rs (known-parseable EPUB3).
fn build_sample_epub() -> Vec<u8> {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        )
        .unwrap();

        zip.start_file("OEBPS/content.opf", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="BookId"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Delete Me Book</dc:title><dc:creator>Test Author</dc:creator><dc:language>en</dc:language><dc:identifier id="BookId">urn:uuid:del-001</dc:identifier></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#,
        )
        .unwrap();

        zip.start_file("OEBPS/nav.xhtml", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Nav</title></head><body><nav epub:type="toc"><ol><li><a href="c1.xhtml">One</a></li></ol></nav></body></html>"#,
        )
        .unwrap();

        zip.start_file("OEBPS/c1.xhtml", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>One</title></head><body><h1>One</h1><p>Hello delete test.</p></body></html>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    buf
}
