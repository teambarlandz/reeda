/// XHTML to DocumentModel parser.
///
/// Parses EPUB XHTML content using `roxmltree` and maps it to the
/// structured `DocumentModel` with typed blocks and inline markup.
///
/// See [EPUB_SPEC.md section 3](../../docs/EPUB_SPEC.md).
use crate::document::{Block, Chapter, DocumentModel, HeadingLevel, ImageRef, Inline};
use crate::error::{EpubError, EpubResult};

/// Parse a single XHTML file into a `Chapter`.
pub fn parse_xhtml(xml: &str, spine_index: u32, href: &str) -> EpubResult<Chapter> {
    let cleaned = crate::error::strip_doctype(xml);
    let doc = roxmltree::Document::parse(&cleaned)
        .map_err(|e| EpubError::InvalidContent(format!("XHTML parse error in {href}: {e}")))?;

    let root = doc.root_element();

    let body = find_body(root)
        .ok_or_else(|| EpubError::InvalidContent(format!("{href}: no <body> element")))?;

    let mut blocks = Vec::new();
    let mut title = String::new();

    for child in body.children() {
        if !child.is_element() {
            continue;
        }
        if let Some(block) = parse_block(child) {
            if title.is_empty() {
                if let Block::Heading(_level, ref inlines) = block {
                    title = inline_to_text_plain(inlines);
                }
            }
            blocks.push(block);
        }
    }

    Ok(Chapter {
        spine_index,
        title,
        href: href.to_string(),
        blocks,
    })
}

/// Parse multiple XHTML files into a `DocumentModel`.
pub fn parse_chapters(contents: &[(&str, &str, u32)]) -> EpubResult<DocumentModel> {
    let mut chapters = Vec::new();
    for (href, xml, spine_index) in contents {
        let chapter = parse_xhtml(xml, *spine_index, href)?;
        chapters.push(chapter);
    }
    Ok(DocumentModel { chapters })
}

fn find_body<'a>(el: roxmltree::Node<'a, 'a>) -> Option<roxmltree::Node<'a, 'a>> {
    if el.is_element() && el.tag_name().name() == "body" {
        return Some(el);
    }
    for child in el.children() {
        if let Some(found) = find_body(child) {
            return Some(found);
        }
    }
    None
}

fn parse_block(el: roxmltree::Node) -> Option<Block> {
    let tag = el.tag_name().name();
    match tag {
        "h1" => Some(Block::Heading(HeadingLevel::H1, parse_inlines(el))),
        "h2" => Some(Block::Heading(HeadingLevel::H2, parse_inlines(el))),
        "h3" => Some(Block::Heading(HeadingLevel::H3, parse_inlines(el))),
        "h4" => Some(Block::Heading(HeadingLevel::H4, parse_inlines(el))),
        "h5" => Some(Block::Heading(HeadingLevel::H5, parse_inlines(el))),
        "h6" => Some(Block::Heading(HeadingLevel::H6, parse_inlines(el))),
        "p" => Some(Block::Paragraph(parse_inlines(el))),
        "blockquote" => Some(Block::Blockquote(parse_inlines(el))),
        "li" => Some(Block::ListItem(parse_inlines(el))),
        "pre" => Some(Block::CodeBlock(extract_text(el))),
        "hr" => Some(Block::HorizontalRule),
        "img" => {
            let src = el.attribute("src").unwrap_or("").to_string();
            let alt = el.attribute("alt").unwrap_or("").to_string();
            let width = el.attribute("width").map(String::from);
            let height = el.attribute("height").map(String::from);
            if src.is_empty() {
                return None;
            }
            Some(Block::Image(ImageRef {
                path: src,
                alt,
                width,
                height,
            }))
        }
        "div" | "section" | "article" | "figure" | "figcaption" => {
            let children = parse_inlines(el);
            if children.is_empty() {
                None
            } else {
                Some(Block::Paragraph(children))
            }
        }
        "script" | "style" | "form" | "iframe" | "video" | "audio" | "svg" | "canvas" => None,
        _ => {
            let inlines = parse_inlines(el);
            if inlines.is_empty() {
                None
            } else {
                Some(Block::Paragraph(inlines))
            }
        }
    }
}

fn parse_inlines(el: roxmltree::Node) -> Vec<Inline> {
    let mut result = Vec::new();
    for child in el.children() {
        if child.is_text() {
            if let Some(text) = child.text() {
                let t = text.trim().to_string();
                if !t.is_empty() {
                    result.push(Inline::Text(t));
                }
            }
        } else if child.is_element() {
            let tag = child.tag_name().name();
            match tag {
                "em" | "i" => {
                    let children = parse_inlines(child);
                    if !children.is_empty() {
                        result.push(Inline::Emphasis(children));
                    }
                }
                "strong" | "b" => {
                    let children = parse_inlines(child);
                    if !children.is_empty() {
                        result.push(Inline::Strong(children));
                    }
                }
                "u" => {
                    let children = parse_inlines(child);
                    if !children.is_empty() {
                        result.push(Inline::Underline(children));
                    }
                }
                "s" | "del" => {
                    let children = parse_inlines(child);
                    if !children.is_empty() {
                        result.push(Inline::Strikethrough(children));
                    }
                }
                "code" => {
                    result.push(Inline::Code(extract_text(child)));
                }
                "a" => {
                    let href = child.attribute("href").unwrap_or("").to_string();
                    let children = parse_inlines(child);
                    result.push(Inline::Link { href, children });
                }
                "br" => {
                    result.push(Inline::Break);
                }
                "sub" => {
                    let children = parse_inlines(child);
                    if !children.is_empty() {
                        result.push(Inline::Sub(children));
                    }
                }
                "sup" => {
                    let children = parse_inlines(child);
                    if !children.is_empty() {
                        result.push(Inline::Sup(children));
                    }
                }
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    result.extend(parse_inlines(child));
                    result.push(Inline::Break);
                }
                "img" => {
                    let alt = child.attribute("alt").unwrap_or("[image]");
                    result.push(Inline::Text(alt.to_string()));
                }
                _ => {
                    result.extend(parse_inlines(child));
                }
            }
        }
    }
    result
}

fn extract_text(el: roxmltree::Node) -> String {
    let mut result = String::new();
    for child in el.children() {
        if child.is_text() {
            if let Some(text) = child.text() {
                result.push_str(text);
            }
        } else if child.is_element() {
            result.push_str(&extract_text(child));
        }
    }
    result
}

fn inline_to_text_plain(inline: &[Inline]) -> String {
    let mut result = String::new();
    for item in inline {
        match item {
            Inline::Text(t) => result.push_str(t),
            Inline::Strong(c)
            | Inline::Emphasis(c)
            | Inline::Underline(c)
            | Inline::Strikethrough(c)
            | Inline::Sub(c)
            | Inline::Sup(c) => result.push_str(&inline_to_text_plain(c)),
            Inline::Link { children, .. } => result.push_str(&inline_to_text_plain(children)),
            Inline::Code(s) => result.push_str(s),
            Inline::Break => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_XHTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body>
  <h1>Chapter 1: The Beginning</h1>
  <p>It was a <strong>dark</strong> and <em>stormy</em> night.</p>
  <p>Another paragraph with a <a href="footnote1.xhtml">footnote</a>.</p>
  <img src="images/picture.jpg" alt="A picture"/>
  <hr/>
  <pre>    code block
    here</pre>
</body>
</html>"#;

    #[test]
    fn parse_simple_xhtml() {
        let chapter = parse_xhtml(SIMPLE_XHTML, 0, "chapter1.xhtml").unwrap();
        assert_eq!(chapter.title, "Chapter 1: The Beginning");
        assert!(chapter.blocks.len() >= 4);
    }

    #[test]
    fn heading_extracted() {
        let chapter = parse_xhtml(SIMPLE_XHTML, 0, "ch1.xhtml").unwrap();
        assert!(matches!(
            &chapter.blocks[0],
            Block::Heading(HeadingLevel::H1, _)
        ));
    }

    #[test]
    fn inline_strong_italic() {
        let chapter = parse_xhtml(SIMPLE_XHTML, 0, "ch1.xhtml").unwrap();
        if let Block::Paragraph(ref inlines) = chapter.blocks[1] {
            assert!(inlines.iter().any(|i| matches!(i, Inline::Strong(_))));
            assert!(inlines.iter().any(|i| matches!(i, Inline::Emphasis(_))));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn link_parsed() {
        let chapter = parse_xhtml(SIMPLE_XHTML, 0, "ch1.xhtml").unwrap();
        if let Block::Paragraph(ref inlines) = chapter.blocks[2] {
            assert!(inlines.iter().any(|i| matches!(i, Inline::Link { .. })));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn image_parsed() {
        let chapter = parse_xhtml(SIMPLE_XHTML, 0, "ch1.xhtml").unwrap();
        let has_image = chapter.blocks.iter().any(|b| matches!(b, Block::Image(_)));
        assert!(has_image);
    }

    #[test]
    fn ignored_elements() {
        let xhtml = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml">
<body><script>var x = 1;</script><p>Hello</p></body></html>"#;
        let chapter = parse_xhtml(xhtml, 0, "test.xhtml").unwrap();
        assert_eq!(chapter.blocks.len(), 1);
        assert!(matches!(&chapter.blocks[0], Block::Paragraph(_)));
    }

    #[test]
    fn no_body_returns_error() {
        let xhtml = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml">
<head><title>T</title></head></html>"#;
        assert!(parse_xhtml(xhtml, 0, "test.xhtml").is_err());
    }

    #[test]
    fn parse_multiple_chapters() {
        let contents = vec![
            ("ch1.xhtml", SIMPLE_XHTML, 0u32),
            ("ch2.xhtml", SIMPLE_XHTML, 1u32),
        ];
        let doc = parse_chapters(&contents).unwrap();
        assert_eq!(doc.chapters.len(), 2);
    }
}
