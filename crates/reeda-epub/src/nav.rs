/// Navigation document parser (EPUB3 nav.xhtml + EPUB2 toc.ncx).
///
/// Produces a unified `TableOfContents` tree from either format.
///
/// See [EPUB_SPEC.md §2](../../docs/EPUB_SPEC.md).
use crate::error::{EpubError, EpubResult};

/// A table of contents tree.
#[derive(Debug, Clone)]
pub struct TableOfContents {
    /// Top-level navigation items.
    pub items: Vec<TocItem>,
}

/// A single TOC entry.
#[derive(Debug, Clone)]
pub struct TocItem {
    /// Display label.
    pub label: String,
    /// Resolved href (relative to OPF base dir).
    pub href: String,
    /// Nested children (for nested nav).
    pub children: Vec<TocItem>,
}

/// Parse an EPUB3 `nav.xhtml` document.
///
/// Looks for `<nav epub:type="toc">` and extracts `<ol>/<li>/<a>` structure.
pub fn parse_nav_xhtml(xml: &str) -> EpubResult<TableOfContents> {
    let cleaned = crate::error::strip_doctype(xml);
    let doc = roxmltree::Document::parse(&cleaned)
        .map_err(|e| EpubError::InvalidNav(format!("XML parse error: {e}")))?;

    // Find <nav epub:type="toc"> or <nav type="toc">.
    let nav = find_toc_nav(doc.root_element())
        .ok_or_else(|| EpubError::InvalidNav("no <nav epub:type=\"toc\"> found".into()))?;

    let ol = nav
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "ol")
        .ok_or_else(|| EpubError::InvalidNav("<nav> missing <ol>".into()))?;

    let items = parse_ol(ol);

    Ok(TableOfContents { items })
}

fn find_toc_nav<'a>(el: roxmltree::Node<'a, 'a>) -> Option<roxmltree::Node<'a, 'a>> {
    if el.is_element() && el.tag_name().name() == "nav" {
        for attr in el.attributes() {
            let name = attr.name();
            if (name == "type" || name.ends_with(":type")) && attr.value() == "toc" {
                return Some(el);
            }
        }
    }
    for child in el.children() {
        if let Some(found) = find_toc_nav(child) {
            return Some(found);
        }
    }
    None
}

fn parse_ol<'a>(ol: roxmltree::Node<'a, 'a>) -> Vec<TocItem> {
    let mut items = Vec::new();
    for li in ol
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "li")
    {
        if let Some(item) = parse_li(li) {
            items.push(item);
        }
    }
    items
}

fn parse_li(li: roxmltree::Node) -> Option<TocItem> {
    let mut label = String::new();
    let mut href = None;
    let mut children = Vec::new();

    for child in li.children() {
        if !child.is_element() {
            continue;
        }
        match child.tag_name().name() {
            "a" => {
                href = child.attribute("href").map(String::from);
                label = child.text().unwrap_or("").trim().to_string();
            }
            "span" => {
                if label.is_empty() {
                    label = child.text().unwrap_or("").trim().to_string();
                }
            }
            "ol" => {
                children = parse_ol(child);
            }
            _ => {}
        }
    }

    let href = href?;
    if label.is_empty() {
        label = href.clone();
    }

    Some(TocItem {
        label,
        href,
        children,
    })
}

/// Parse an EPUB2 `toc.ncx` document.
pub fn parse_ncx(xml: &str) -> EpubResult<TableOfContents> {
    let cleaned = crate::error::strip_doctype(xml);
    let doc = roxmltree::Document::parse(&cleaned)
        .map_err(|e| EpubError::InvalidNav(format!("NCX parse error: {e}")))?;

    let root = doc.root_element();

    // Find <navMap>
    let nav_map = root
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "navMap")
        .ok_or_else(|| EpubError::InvalidNav("NCX missing <navMap>".into()))?;

    let items = parse_nav_points(nav_map);

    Ok(TableOfContents { items })
}

fn parse_nav_points(parent: roxmltree::Node) -> Vec<TocItem> {
    let mut items = Vec::new();
    for np in parent
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "navPoint")
    {
        if let Some(item) = parse_nav_point(np) {
            items.push(item);
        }
    }
    items
}

fn parse_nav_point(np: roxmltree::Node) -> Option<TocItem> {
    let label = np
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "navLabel")
        .and_then(|nl| {
            nl.children()
                .find(|c| c.is_element() && c.tag_name().name() == "text")
        })
        .and_then(|t| t.text())
        .unwrap_or("")
        .trim()
        .to_string();

    let href = np
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "content")
        .and_then(|c| c.attribute("src"))
        .unwrap_or("")
        .to_string();

    if href.is_empty() {
        return None;
    }

    let children: Vec<TocItem> = np
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "navPoint")
        .filter_map(parse_nav_point)
        .collect();

    Some(TocItem {
        label,
        href,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAV_XHTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Navigation</title></head>
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="chapter1.xhtml">Chapter 1</a></li>
      <li>
        <a href="chapter2.xhtml">Chapter 2</a>
        <ol>
          <li><a href="chapter2.xhtml#section1">Section 1</a></li>
          <li><a href="chapter2.xhtml#section2">Section 2</a></li>
        </ol>
      </li>
      <li><a href="chapter3.xhtml">Chapter 3</a></li>
    </ol>
  </nav>
</body>
</html>"#;

    #[test]
    fn parse_nav_flat_and_nested() {
        let toc = parse_nav_xhtml(NAV_XHTML).unwrap();
        assert_eq!(toc.items.len(), 3);
        assert_eq!(toc.items[0].label, "Chapter 1");
        assert_eq!(toc.items[0].href, "chapter1.xhtml");
        assert!(toc.items[0].children.is_empty());

        assert_eq!(toc.items[1].label, "Chapter 2");
        assert_eq!(toc.items[1].children.len(), 2);
        assert_eq!(toc.items[1].children[0].label, "Section 1");
        assert_eq!(toc.items[1].children[0].href, "chapter2.xhtml#section1");
    }

    const NCX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="ch1" playOrder="1">
      <navLabel><text>Chapter 1</text></navLabel>
      <content src="chapter1.xhtml"/>
    </navPoint>
    <navPoint id="ch2" playOrder="2">
      <navLabel><text>Chapter 2</text></navLabel>
      <content src="chapter2.xhtml"/>
      <navPoint id="ch2s1" playOrder="3">
        <navLabel><text>Section 2.1</text></navLabel>
        <content src="chapter2.xhtml#s1"/>
      </navPoint>
    </navPoint>
  </navMap>
</ncx>"#;

    #[test]
    fn parse_ncx_basic() {
        let toc = parse_ncx(NCX_XML).unwrap();
        assert_eq!(toc.items.len(), 2);
        assert_eq!(toc.items[0].label, "Chapter 1");
        assert_eq!(toc.items[0].href, "chapter1.xhtml");

        assert_eq!(toc.items[1].children.len(), 1);
        assert_eq!(toc.items[1].children[0].label, "Section 2.1");
        assert_eq!(toc.items[1].children[0].href, "chapter2.xhtml#s1");
    }

    #[test]
    fn nav_no_toc_returns_error() {
        let xml = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><nav epub:type="landmarks"></nav></body></html>"#;
        assert!(parse_nav_xhtml(xml).is_err());
    }

    #[test]
    fn ncx_no_navmap_returns_error() {
        let xml = r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"/>"#;
        assert!(parse_ncx(xml).is_err());
    }
}
