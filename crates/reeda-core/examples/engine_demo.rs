//! Headless engine proof for Windows (no UI involved).
//!
//! Exercises the full backend stack end-to-end and prints a summary:
//!   1. Fresh data root: BookStore + SQLite Database + Tantivy SearchService
//!   2. EPUB import (generated in-memory) → pagination → chapter list
//!   3. Page turns (content actually changes)
//!   4. PDF import (a real stored book.pdf) → page count/sizes/outline
//!   5. PDFium rasterization of page 1 → PNG written to dist/engine_demo/
//!   6. Full-text search across the library
//!
//! Run from the repo root:
//!   cargo run -p reeda-core --example engine_demo

use std::fs;
use std::path::{Path, PathBuf};

use reeda_core::{App, BookStore, Command, Database, Event};
use reeda_pdf::render::render_page;
use reeda_pdf::theme::Theme;

fn main() {
    if let Err(e) = run() {
        eprintln!("ENGINE DEMO FAILED: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf();

    // PDFium: reuse the vendored DLL unless the caller picked one.
    if std::env::var_os("PDFIUM_LIBRARY_PATH").is_none() {
        let dll = repo.join("third_party/pdfium/win-x64/pdfium.dll");
        std::env::set_var("PDFIUM_LIBRARY_PATH", &dll);
    }

    // ── 1. Fresh persistent stack ────────────────────────────────────
    let base = std::env::temp_dir()
        .join("opencode")
        .join("engine_demo")
        .join("reeda_data");
    if base.exists() {
        fs::remove_dir_all(&base).map_err(|e| format!("clear data root: {e}"))?;
    }
    let store = BookStore::new(&base).map_err(|e| format!("BookStore: {e}"))?;
    let db = Database::open(store.root().join("reeda.sqlite"))
        .map_err(|e| format!("Database: {e}"))?;
    let mut app = App::with_store_db(store, db);
    let search =
        reeda_core::search::SearchService::open(&base).map_err(|e| format!("Search: {e}"))?;
    app.set_search(search);
    println!("[1] data root      : {}", base.display());

    // ── 2. EPUB import + pagination ──────────────────────────────────
    let epub_bytes = build_sample_epub();
    let events = app.import_from_bytes(epub_bytes, "engine-demo.epub".into());
    let epub_id = expect_import(&events)?;
    app.dispatch(Command::OpenBook { book_id: epub_id });
    let snap = app.snapshot();
    println!(
        "[2] EPUB imported  : \"{}\" by {}",
        snap.current_book.as_ref().map_or("?", |b| b.title.as_str()),
        snap.current_book
            .as_ref()
            .and_then(|b| b.author.clone())
            .unwrap_or_else(|| "?".into())
    );
    println!(
        "    pages={} chapters=[{}]",
        snap.total_pages,
        snap.current_chapters
            .iter()
            .map(|c| c.title.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("    page 1 text     : {:.60}…", snap.page_text);

    // ── 3. Page turns ────────────────────────────────────────────────
    app.dispatch(Command::TurnPage { forward: true });
    app.dispatch(Command::TurnPage { forward: true });
    let snap = app.snapshot();
    println!(
        "[3] after 2 turns  : page {}/{} text {:.40}…",
        snap.current_page + 1,
        snap.total_pages,
        snap.page_text
    );

    // ── 4. PDF import ────────────────────────────────────────────────
    let pdf_path = find_real_pdf(&repo).ok_or(
        "no real PDF found (looked at $REEDA_DEMO_PDF and target/release/reeda_data/books/*/book.pdf)",
    )?;
    let events = app.dispatch(Command::ImportPdf {
        path: pdf_path.display().to_string(),
    });
    let pdf_id = expect_import(&events)?;
    app.dispatch(Command::OpenBook { book_id: pdf_id });
    let snap = app.snapshot();
    let pdf = snap.pdf.as_ref().ok_or("OpenBook produced no PdfView")?;
    println!(
        "[4] PDF imported   : {} ({} pages, first page {}x{} pt)",
        pdf_path
            .file_name()
            .map_or_else(|| "?".to_string(), |n| n.to_string_lossy().into_owned()),
        pdf.page_count,
        pdf.page_sizes.first().map_or(0.0, |s| s.0),
        pdf.page_sizes.first().map_or(0.0, |s| s.1),
    );
    println!(
        "    outline entries : {}{}",
        pdf.outline.len(),
        if pdf.outline.is_empty() {
            String::new()
        } else {
            format!(
                " (first: {:?} → page {})",
                pdf.outline[0].title, pdf.outline[0].page_index
            )
        }
    );

    // ── 5. Rasterize page 1 → PNG ────────────────────────────────────
    let page = render_page(&pdf_path, 0, 1.0, Theme::Normal)
        .map_err(|e| format!("render_page: {e}"))?;
    let out_dir = repo.join("dist").join("engine_demo");
    fs::create_dir_all(&out_dir).map_err(|e| format!("mkdir: {e}"))?;
    let png_path = out_dir.join("pdf_page1.png");
    write_png(&png_path, page.width, page.height, &page.rgba)?;
    println!(
        "[5] rasterized p1  : {}x{} px → {} ({} KiB)",
        page.width,
        page.height,
        png_path.display(),
        fs::metadata(&png_path).map(|m| m.len() / 1024).unwrap_or(0)
    );

    // ── 6. Full-text search ──────────────────────────────────────────
    let hits = app.search_books("zebra", None).ok_or("search unavailable")?;
    println!(
        "[6] search 'zebra' : {} hit(s)",
        hits.hits.len()
    );
    if let Some(hit) = hits.hits.first() {
        println!(
            "    top hit         : [{}] {} — {}",
            hit.title, hit.chapter_title, hit.snippet
        );
    }

    println!("\nALL ENGINE CHECKS PASSED");
    Ok(())
}

fn expect_import(events: &[Event]) -> Result<reeda_core::BookId, String> {
    for ev in events {
        match ev {
            Event::ImportFinished { book_id } => return Ok(*book_id),
            Event::ImportFailed { error } => return Err(format!("import failed: {error}")),
            other => return Err(format!("unexpected event: {other:?}")),
        }
    }
    Err("no events returned from import".into())
}

fn find_real_pdf(repo: &Path) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("REEDA_DEMO_PDF") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let glob_base = repo.join("target/release/reeda_data/books");
    let mut entries: Vec<PathBuf> = fs::read_dir(glob_base)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path().join("book.pdf"))
        .filter(|p| p.is_file())
        .collect();
    entries.sort();
    entries.into_iter().next()
}

/// Minimal valid EPUB3 with three chapters of real prose (searchable).
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
            br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="BookId"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Engine Demo Book</dc:title><dc:creator>Reeda CI</dc:creator><dc:language>en</dc:language><dc:identifier id="BookId">urn:uuid:engine-demo-001</dc:identifier></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/><item id="ch2" href="chapter2.xhtml" media-type="application/xhtml+xml"/><item id="ch3" href="chapter3.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="ch1"/><itemref idref="ch2"/><itemref idref="ch3"/></spine></package>"#,
        )
        .unwrap();

        zip.start_file("OEBPS/nav.xhtml", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Navigation</title></head><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">Origins</a></li><li><a href="chapter2.xhtml">The Zebra Cipher</a></li><li><a href="chapter3.xhtml">Resolution</a></li></ol></nav></body></html>"#,
        )
        .unwrap();

        let para = |topic: &str| {
            format!(
                "<p>{topic} The zebra trotted across the plain while the engine hummed \
                 beneath the hood of the old truck. Dust settled over everything.</p>"
            )
        };
        let chapter = |title: &str, topic: &str| {
            format!(
                r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>{title}</title></head><body><h1>{title}</h1>{}{}{}</body></html>"#,
                para(topic),
                para(topic),
                para(topic)
            )
        };
        for (file, title, topic) in [
            ("chapter1.xhtml", "Origins", "It began on a Tuesday."),
            ("chapter2.xhtml", "The Zebra Cipher", "The message made no sense."),
            ("chapter3.xhtml", "Resolution", "Everything ended quietly."),
        ] {
            zip.start_file(format!("OEBPS/{file}"), deflated).unwrap();
            zip.write_all(chapter(title, topic).as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    buf
}

// ── Minimal dependency-free PNG writer (RGBA, color type 6) ─────────

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!(
            "pixel buffer {} B != {}x{}x4",
            rgba.len(),
            width,
            height
        ));
    }

    // Raw scanlines with filter byte 0.
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0u8);
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }

    let mut png = Vec::with_capacity(raw.len() / 2 + 128);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    push_chunk(&mut png, b"IEND", &[]);

    fs::write(path, png).map_err(|e| format!("write {}: {e}", path.display()))
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = crc32_init();
    crc = crc32_push(crc, kind);
    crc = crc32_push(crc, data);
    out.extend_from_slice(&crc32_finish(crc).to_be_bytes());
}

/// zlib stream using stored (uncompressed) deflate blocks — valid, fast, tiny.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut z = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 16);
    z.extend_from_slice(&[0x78, 0x01]); // CMF/FLG: deflate, 32K window
    let mut chunks = data.chunks(65535).peekable();
    if data.is_empty() {
        z.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    while let Some(chunk) = chunks.next() {
        let last = chunks.peek().is_none();
        z.push(u8::from(last)); // BFINAL + BTYPE=00
        let len = chunk.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(chunk);
    }
    z.extend_from_slice(&adler32(data).to_be_bytes());
    z
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32_init() -> u32 {
    0xFFFF_FFFF
}

fn crc32_push(mut state: u32, data: &[u8]) -> u32 {
    for &byte in data {
        state ^= byte as u32;
        for _ in 0..8 {
            let mask = u32::from(state & 1 != 0).wrapping_neg();
            state = (state >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    state
}

fn crc32_finish(state: u32) -> u32 {
    !state
}
