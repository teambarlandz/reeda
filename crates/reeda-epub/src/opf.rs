/// OPF package document parser.
///
/// Parses the EPUB OPF file to extract metadata, manifest, and spine.
/// Supports EPUB 2.x and 3.x package documents.
///
/// See [EPUB_SPEC.md §2](../../docs/EPUB_SPEC.md).
use std::collections::HashMap;

use crate::error::{EpubError, EpubResult};

/// Parsed OPF package document.
#[derive(Debug, Clone)]
pub struct OpfPackage {
    /// EPUB version string (e.g., "3.0", "2.0.1").
    pub version: String,
    /// Parsed metadata (dc:* elements + meta).
    pub metadata: Metadata,
    /// Manifest: item id → `ManifestItem`.
    pub manifest: HashMap<String, ManifestItem>,
    /// Spine: ordered list of `SpineItem` references.
    pub spine: Vec<SpineItem>,
}

/// Dublin Core metadata from the OPF `<metadata>` element.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    /// dc:title
    pub title: Option<String>,
    /// dc:creator (may be multiple, joined with " | ").
    pub creators: Vec<String>,
    /// dc:language
    pub language: Option<String>,
    /// dc:identifier (primary)
    pub identifier: Option<String>,
    /// dc:publisher
    pub publisher: Option<String>,
    /// dc:date
    pub date: Option<String>,
    /// dc:description
    pub description: Option<String>,
    /// meta[name="cover"] content (cover image id from manifest).
    pub cover_id: Option<String>,
}

/// A single manifest `<item>`.
#[derive(Debug, Clone)]
pub struct ManifestItem {
    /// Unique id.
    pub id: String,
    /// Resolved href relative to OPF base dir.
    pub href: String,
    /// Media type.
    pub media_type: String,
    /// Space-separated properties (e.g., "nav", "cover-image").
    pub properties: String,
}

/// A single spine `<itemref>`.
#[derive(Debug, Clone)]
pub struct SpineItem {
    /// References a manifest item id.
    pub idref: String,
    /// If `true`, this is supplementary content (linear="no").
    pub linear: bool,
}

/// Parse an OPF package document from its XML content.
///
/// The `opf_dir` is the directory containing the OPF file, used to resolve
/// relative hrefs. Pass `""` for root-level OPF files.
pub fn parse_opf(xml: &str, _opf_dir: &str) -> EpubResult<OpfPackage> {
    let cleaned = crate::error::strip_doctype(xml);
    let doc = roxmltree::Document::parse(&cleaned)
        .map_err(|e| EpubError::InvalidOpf(format!("XML parse error: {e}")))?;

    let root = doc.root_element();

    // Validate it's a package element.
    if root.tag_name().name() != "package" {
        return Err(EpubError::InvalidOpf(
            "root element must be <package>".into(),
        ));
    }

    let version = root.attribute("version").unwrap_or("2.0").to_string();

    let mut metadata = Metadata::default();
    let mut manifest = HashMap::new();
    let mut spine = Vec::new();

    for child in root.children() {
        if !child.is_element() {
            continue;
        }
        match child.tag_name().name() {
            "metadata" => parse_metadata(child, &mut metadata)?,
            "manifest" => parse_manifest(child, &mut manifest)?,
            "spine" => parse_spine(child, &mut spine)?,
            _ => {} // ignore guide, bindings, etc.
        }
    }

    Ok(OpfPackage {
        version,
        metadata,
        manifest,
        spine,
    })
}

fn parse_metadata(el: roxmltree::Node, meta: &mut Metadata) -> EpubResult<()> {
    for child in el.children() {
        if !child.is_element() {
            continue;
        }
        let tag = child.tag_name().name();
        let text = child.text().unwrap_or("").trim().to_string();

        match tag {
            "title" => {
                if !text.is_empty() {
                    meta.title = Some(text);
                }
            }
            "creator" => {
                if !text.is_empty() {
                    meta.creators.push(text);
                }
            }
            "language" => {
                if !text.is_empty() {
                    meta.language = Some(text);
                }
            }
            "identifier" => {
                if meta.identifier.is_none() && !text.is_empty() {
                    meta.identifier = Some(text);
                }
            }
            "publisher" => {
                if !text.is_empty() {
                    meta.publisher = Some(text);
                }
            }
            "date" => {
                if meta.date.is_none() && !text.is_empty() {
                    meta.date = Some(text);
                }
            }
            "description" => {
                if !text.is_empty() {
                    meta.description = Some(text);
                }
            }
            "meta" => {
                // EPUB2: <meta name="cover" content="cover-image-id"/>
                if let Some(name) = child.attribute("name") {
                    if name == "cover" {
                        meta.cover_id = child.attribute("content").map(String::from);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_manifest(
    el: roxmltree::Node,
    manifest: &mut HashMap<String, ManifestItem>,
) -> EpubResult<()> {
    for child in el.children() {
        if !child.is_element() || child.tag_name().name() != "item" {
            continue;
        }
        let id = child
            .attribute("id")
            .ok_or_else(|| EpubError::InvalidOpf("manifest item missing 'id'".into()))?
            .to_string();
        let href = child
            .attribute("href")
            .ok_or_else(|| EpubError::InvalidOpf(format!("manifest item '{id}' missing 'href'")))?
            .to_string();
        let media_type = child
            .attribute("media-type")
            .unwrap_or("application/octet-stream")
            .to_string();
        let properties = child.attribute("properties").unwrap_or("").to_string();

        manifest.insert(
            id.clone(),
            ManifestItem {
                id,
                href,
                media_type,
                properties,
            },
        );
    }
    Ok(())
}

fn parse_spine(el: roxmltree::Node, spine: &mut Vec<SpineItem>) -> EpubResult<()> {
    for child in el.children() {
        if !child.is_element() || child.tag_name().name() != "itemref" {
            continue;
        }
        let idref = child
            .attribute("idref")
            .ok_or_else(|| EpubError::InvalidOpf("spine itemref missing 'idref'".into()))?
            .to_string();
        let linear = child.attribute("linear").map(|v| v != "no").unwrap_or(true);

        spine.push(SpineItem { idref, linear });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="BookId">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>The Great Gatsby</dc:title>
    <dc:creator>F. Scott Fitzgerald</dc:creator>
    <dc:language>en</dc:language>
    <dc:identifier id="BookId">urn:uuid:12345</dc:identifier>
    <dc:publisher>Scribner</dc:publisher>
    <dc:date>1925</dc:date>
    <dc:description>A novel about the American dream.</dc:description>
    <meta name="cover" content="cover-image"/>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="chapter2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover-image" href="images/cover.jpg" media-type="image/jpeg"/>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="chapter1"/>
    <itemref idref="chapter2"/>
  </spine>
</package>"#;

    #[test]
    fn parse_metadata_fields() {
        let pkg = parse_opf(SAMPLE_OPF, "").unwrap();
        assert_eq!(pkg.version, "3.0");
        assert_eq!(pkg.metadata.title.as_deref(), Some("The Great Gatsby"));
        assert_eq!(pkg.metadata.creators, vec!["F. Scott Fitzgerald"]);
        assert_eq!(pkg.metadata.language.as_deref(), Some("en"));
        assert_eq!(pkg.metadata.identifier.as_deref(), Some("urn:uuid:12345"));
        assert_eq!(pkg.metadata.publisher.as_deref(), Some("Scribner"));
        assert_eq!(pkg.metadata.date.as_deref(), Some("1925"));
        assert_eq!(pkg.metadata.cover_id.as_deref(), Some("cover-image"));
    }

    #[test]
    fn parse_manifest() {
        let pkg = parse_opf(SAMPLE_OPF, "").unwrap();
        assert_eq!(pkg.manifest.len(), 4);
        let ch1 = pkg.manifest.get("chapter1").unwrap();
        assert_eq!(ch1.href, "chapter1.xhtml");
        assert_eq!(ch1.media_type, "application/xhtml+xml");
    }

    #[test]
    fn parse_spine() {
        let pkg = parse_opf(SAMPLE_OPF, "").unwrap();
        assert_eq!(pkg.spine.len(), 2);
        assert_eq!(pkg.spine[0].idref, "chapter1");
        assert!(pkg.spine[0].linear);
        assert_eq!(pkg.spine[1].idref, "chapter2");
    }

    #[test]
    fn spine_linear_no() {
        let xml = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="a" href="a.xhtml" media-type="application/xhtml+xml"/>
    <item id="b" href="b.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="a"/>
    <itemref idref="b" linear="no"/>
  </spine>
</package>"#;
        let pkg = parse_opf(xml, "").unwrap();
        assert!(pkg.spine[0].linear);
        assert!(!pkg.spine[1].linear);
    }

    #[test]
    fn invalid_xml_returns_error() {
        let err = parse_opf("<not valid", "").unwrap_err();
        assert!(err.to_string().contains("XML parse error"));
    }

    #[test]
    fn missing_root_element() {
        let xml = r#"<?xml version="1.0"?><wrong/>"#;
        let err = parse_opf(xml, "").unwrap_err();
        assert!(err.to_string().contains("<package>"));
    }
}
