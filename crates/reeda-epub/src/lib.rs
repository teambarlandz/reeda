//! `reeda-epub` — the EPUB parsing & rendering engine of Reeda.
//!
//! Modules (docs/EPUB_SPEC.md, docs/TECHNICAL_DESIGN.md section 2.2):
//! - `container` — ZIP open/validate/read (zip-slip guarded)
//! - `opf` — metadata, manifest, spine
//! - `nav` — nav.xhtml + toc.ncx to TableOfContents
//! - `xhtml` — XHTML to Chapter (block/inline model)
//! - `cfi` — CFI parse/serialize (EPUB_SPEC.md section 7)
//! - `document` — DocumentModel (chapters, blocks, images, links)

#![deny(missing_docs)]

/// EPUB CFI (Canonical Fragment Identifier) position model.
pub mod cfi;
/// EPUB ZIP container reader (zip-slip guarded).
pub mod container;
/// Document model: blocks, inline markup, images.
pub mod document;
/// Error types for EPUB operations.
pub mod error;
/// Navigation document parser (nav.xhtml + toc.ncx).
pub mod nav;
/// OPF package document parser.
pub mod opf;
/// Deterministic paginator: splits a DocumentModel into pages.
pub mod paginator;
/// XHTML to DocumentModel parser.
pub mod xhtml;

/// Returns the current reeda-epub crate version.
pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// High-level EPUB book representation after parsing.
#[derive(Debug)]
pub struct EpubBook {
    /// The OPF package metadata.
    pub opf: opf::OpfPackage,
    /// Table of contents.
    pub toc: nav::TableOfContents,
    /// The document model (all chapters' content).
    pub document: document::DocumentModel,
}

/// Parse a complete EPUB from raw ZIP bytes.
///
/// Opens the container, parses OPF, nav, and all XHTML content.
pub fn open_epub(data: &[u8]) -> error::EpubResult<EpubBook> {
    let container = container::Container::open(data)?;

    let container_xml = container
        .read_str("META-INF/container.xml")
        .map_err(|_| error::EpubError::MissingContainer)?;
    let rootfile_path = parse_container_rootfile(container_xml)?;

    let opf_xml = container
        .read_str(&rootfile_path)
        .map_err(|_| error::EpubError::MissingRootfile)?;
    let opf_dir = rootfile_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let opf = opf::parse_opf(opf_xml, opf_dir)?;

    let toc = parse_navigation(&container, &opf, opf_dir)?;

    let mut contents = Vec::new();
    for (i, spine_item) in opf.spine.iter().enumerate() {
        if let Some(item) = opf.manifest.get(&spine_item.idref) {
            let href = if opf_dir.is_empty() {
                item.href.clone()
            } else {
                format!("{opf_dir}/{}", item.href)
            };
            if let Ok(xml) = container.read_str(&href) {
                contents.push((item.href.as_str(), xml, i as u32));
            }
        }
    }
    let document = xhtml::parse_chapters(&contents)?;

    Ok(EpubBook { opf, toc, document })
}

/// Parse the rootfile path from container.xml.
fn parse_container_rootfile(xml: &str) -> error::EpubResult<String> {
    let cleaned = error::strip_doctype(xml);
    let doc = roxmltree::Document::parse(&cleaned)
        .map_err(|e| error::EpubError::InvalidOpf(format!("container.xml parse error: {e}")))?;

    let root = doc.root_element();
    find_rootfile(root).ok_or(error::EpubError::MissingRootfile)
}

fn find_rootfile<'a>(el: roxmltree::Node<'a, 'a>) -> Option<String> {
    if el.is_element() && el.tag_name().name() == "rootfile" {
        return el.attribute("full-path").map(String::from);
    }
    for child in el.children() {
        if let Some(found) = find_rootfile(child) {
            return Some(found);
        }
    }
    None
}

/// Parse navigation: try EPUB3 nav.xhtml, fall back to EPUB2 toc.ncx.
fn parse_navigation(
    container: &container::Container,
    opf: &opf::OpfPackage,
    opf_dir: &str,
) -> error::EpubResult<nav::TableOfContents> {
    // Try EPUB3: look for nav item (properties="nav") in manifest first.
    let nav_item = opf
        .manifest
        .values()
        .find(|m| m.properties.split_whitespace().any(|p| p == "nav"))
        .or_else(|| {
            opf.manifest
                .values()
                .find(|m| m.media_type == "application/xhtml+xml")
        });

    if let Some(item) = nav_item {
        let href = if opf_dir.is_empty() {
            item.href.clone()
        } else {
            format!("{opf_dir}/{}", item.href)
        };
        if let Ok(xml) = container.read_str(&href) {
            if let Ok(toc) = nav::parse_nav_xhtml(xml) {
                if !toc.items.is_empty() {
                    return Ok(toc);
                }
            }
        }
    }

    // Fall back to EPUB2 ncx.
    let ncx_href = opf
        .manifest
        .values()
        .find(|m| m.media_type == "application/x-dtbncx+xml")
        .map(|m| {
            if opf_dir.is_empty() {
                m.href.clone()
            } else {
                format!("{opf_dir}/{}", m.href)
            }
        });

    if let Some(href) = ncx_href {
        if let Ok(xml) = container.read_str(&href) {
            return nav::parse_ncx(xml);
        }
    }

    Ok(nav::TableOfContents { items: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_test_epub() -> Vec<u8> {
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
            zip.write_all(br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();

            zip.start_file("OEBPS/content.opf", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="BookId"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Test Book</dc:title><dc:creator>Author</dc:creator><dc:language>en</dc:language><dc:identifier id="BookId">urn:uuid:test-001</dc:identifier></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/><item id="ch2" href="chapter2.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="ch1"/><itemref idref="ch2"/></spine></package>"#).unwrap();

            zip.start_file("OEBPS/nav.xhtml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Navigation</title></head><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">Chapter 1</a></li><li><a href="chapter2.xhtml">Chapter 2</a></li></ol></nav></body></html>"#).unwrap();

            zip.start_file("OEBPS/chapter1.xhtml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Ch1</title></head><body><h1>Chapter 1</h1><p>Hello <strong>world</strong>.</p><p>Second paragraph.</p></body></html>"#).unwrap();

            zip.start_file("OEBPS/chapter2.xhtml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Ch2</title></head><body><h1>Chapter 2</h1><p>More <em>content</em> here.</p></body></html>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn open_and_parse_full_epub() {
        let data = make_test_epub();
        let book = open_epub(&data).unwrap();

        assert_eq!(book.opf.metadata.title.as_deref(), Some("Test Book"));
        assert_eq!(book.opf.metadata.creators, vec!["Author"]);

        assert_eq!(book.toc.items.len(), 2);
        assert_eq!(book.toc.items[0].label, "Chapter 1");
        assert_eq!(book.toc.items[1].label, "Chapter 2");

        assert_eq!(book.document.chapters.len(), 2);
        assert_eq!(book.document.chapters[0].title, "Chapter 1");
    }

    #[test]
    fn chapter_content_parsed() {
        let data = make_test_epub();
        let book = open_epub(&data).unwrap();
        let ch1 = &book.document.chapters[0];

        assert!(matches!(
            &ch1.blocks[0],
            document::Block::Heading(document::HeadingLevel::H1, _)
        ));

        if let document::Block::Paragraph(ref inlines) = ch1.blocks[1] {
            assert!(inlines
                .iter()
                .any(|i| matches!(i, document::Inline::Strong(_))));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn missing_rootfile() {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            let deflated = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("META-INF/container.xml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles/></container>"#).unwrap();
            zip.finish().unwrap();
        }
        let err = open_epub(&buf).unwrap_err();
        assert!(err.to_string().contains("rootfile"));
    }
}
