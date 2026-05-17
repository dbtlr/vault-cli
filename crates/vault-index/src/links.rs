use std::collections::HashMap;
use std::ops::Range;
use std::path::Component;

use camino::{Utf8Path, Utf8PathBuf};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use regex::Regex;
use vault_core::{
    Document, Heading, Link, LinkKind, LinkSourceArea, LinkSourceContext, LinkStatus, SourceSpan,
    UnresolvedReason, VaultFile,
};

use vault_frontmatter::frontmatter_property_strings;

pub(crate) fn parse_commonmark(
    source_path: &Utf8Path,
    content: &str,
    body: &str,
    body_start: usize,
) -> (Vec<Heading>, Vec<Link>) {
    let parser = Parser::new(body).into_offset_iter();
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut active_heading: Option<(u8, String, usize)> = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                active_heading = Some((
                    heading_level(level),
                    String::new(),
                    body_start + range.start,
                ));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text, start)) = active_heading.take() {
                    let text = text.trim().to_string();
                    headings.push(Heading {
                        level,
                        slug: slugify(&text),
                        text,
                        source_span: Some(source_span(content, start)),
                    });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, heading_text, _)) = active_heading.as_mut() {
                    heading_text.push_str(&text);
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let raw = dest_url.to_string();
                if is_local_markdown_target(&raw) {
                    let (target, anchor) = split_anchor(&decode_percent_escapes(&raw));
                    links.push(Link {
                        source_path: source_path.to_path_buf(),
                        raw,
                        kind: LinkKind::Markdown,
                        target,
                        label: None,
                        anchor,
                        block_ref: None,
                        source_span: Some(source_span(content, body_start + range.start)),
                        source_context: Some(LinkSourceContext {
                            area: LinkSourceArea::Body,
                            property: None,
                        }),
                        resolved_path: None,
                        unresolved_reason: None,
                        candidates: Vec::new(),
                        status: LinkStatus::Unresolved,
                    });
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let raw = dest_url.to_string();
                if is_local_file_target(&raw) {
                    let (target, anchor) = split_anchor(&decode_percent_escapes(&raw));
                    links.push(Link {
                        source_path: source_path.to_path_buf(),
                        raw,
                        kind: LinkKind::Embed,
                        target,
                        label: None,
                        anchor,
                        block_ref: None,
                        source_span: Some(source_span(content, body_start + range.start)),
                        source_context: Some(LinkSourceContext {
                            area: LinkSourceArea::Body,
                            property: None,
                        }),
                        resolved_path: None,
                        unresolved_reason: None,
                        candidates: Vec::new(),
                        status: LinkStatus::Unresolved,
                    });
                }
            }
            _ => {}
        }
    }

    (headings, links)
}

pub(crate) fn parse_wikilinks(
    source_path: &Utf8Path,
    content: &str,
    body: &str,
    body_start: usize,
) -> Vec<Link> {
    let ignored_ranges = ignored_wikilink_ranges(body);
    parse_wikilinks_in_text(
        source_path,
        body,
        Some((content, body_start, ignored_ranges)),
        Some(LinkSourceContext {
            area: LinkSourceArea::Body,
            property: None,
        }),
    )
}

fn parse_wikilinks_in_text(
    source_path: &Utf8Path,
    text: &str,
    source: Option<(&str, usize, Vec<Range<usize>>)>,
    source_context: Option<LinkSourceContext>,
) -> Vec<Link> {
    let wikilink_re = Regex::new(r"(!?)\[\[([^\]]+)\]\]").expect("valid wikilink regex");
    wikilink_re
        .captures_iter(text)
        .filter_map(|captures| {
            let full_match = captures.get(0)?;
            if source.as_ref().is_some_and(|(_, _, ignored_ranges)| {
                ignored_ranges
                    .iter()
                    .any(|range| ranges_overlap(range, &(full_match.start()..full_match.end())))
            }) {
                return None;
            }

            let source_span = source.as_ref().map(|(content, base_offset, _)| {
                source_span(content, base_offset + full_match.start())
            });

            let raw = full_match.as_str().to_string();
            let is_embed = captures.get(1).is_some_and(|m| m.as_str() == "!");
            let inner = captures.get(2)?.as_str();
            let (target_part, label) = inner
                .split_once('|')
                .map_or((inner, None), |(target, label)| {
                    (target, Some(label.trim().to_string()))
                });
            let (target, anchor, block_ref) = split_anchor_or_block_ref(target_part.trim());

            Some(Link {
                source_path: source_path.to_path_buf(),
                raw,
                kind: if is_embed {
                    LinkKind::Embed
                } else {
                    LinkKind::Wikilink
                },
                target,
                label,
                anchor,
                block_ref,
                source_span,
                source_context: source_context.clone(),
                resolved_path: None,
                unresolved_reason: None,
                candidates: Vec::new(),
                status: LinkStatus::Unresolved,
            })
        })
        .collect()
}

pub(crate) fn parse_frontmatter_wikilinks(
    source_path: &Utf8Path,
    content: &str,
    frontmatter_range: Option<Range<usize>>,
    frontmatter: &serde_json::Value,
) -> Vec<Link> {
    let Some(object) = frontmatter.as_object() else {
        return Vec::new();
    };

    frontmatter_property_strings(object, content, frontmatter_range)
        .into_iter()
        .flat_map(|property_string| {
            parse_wikilinks_in_text(
                source_path,
                property_string.text,
                property_string
                    .offset
                    .map(|offset| (content, offset, Vec::new())),
                Some(LinkSourceContext {
                    area: LinkSourceArea::Frontmatter,
                    property: Some(property_string.property),
                }),
            )
        })
        .collect()
}

fn ignored_wikilink_ranges(body: &str) -> Vec<Range<usize>> {
    let parser = Parser::new(body).into_offset_iter();
    let mut ignored_ranges = Vec::new();
    let mut active_code_block_start = None;

    for (event, range) in parser {
        match event {
            Event::Code(_) => ignored_ranges.push(range),
            Event::Start(Tag::CodeBlock(_)) => active_code_block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = active_code_block_start.take() {
                    ignored_ranges.push(start..range.end);
                }
            }
            _ => {}
        }
    }

    ignored_ranges
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

pub(crate) fn parse_block_ids(body: &str) -> Vec<String> {
    let block_re = Regex::new(r"(?:^|\s)\^([A-Za-z0-9_-]+)\s*$").expect("valid block id regex");
    body.lines()
        .filter_map(|line| {
            block_re
                .captures(line)
                .and_then(|captures| captures.get(1))
                .map(|block_id| block_id.as_str().to_string())
        })
        .collect()
}

pub(crate) fn resolve_links(files: &[VaultFile], documents: &mut [Document]) {
    let mut by_path: HashMap<String, Utf8PathBuf> = HashMap::new();
    let mut by_path_lower: HashMap<String, Utf8PathBuf> = HashMap::new();
    let mut by_stem: HashMap<String, Vec<Utf8PathBuf>> = HashMap::new();
    let mut facts_by_path: HashMap<Utf8PathBuf, DocumentFacts> = HashMap::new();

    for file in files {
        by_path.insert(file.path.as_str().to_string(), file.path.clone());
        by_path_lower.insert(file.path.as_str().to_lowercase(), file.path.clone());
    }

    for document in documents.iter() {
        by_stem
            .entry(document.stem.to_lowercase())
            .or_default()
            .push(document.path.clone());
        facts_by_path.insert(
            document.path.clone(),
            DocumentFacts {
                heading_slugs: document
                    .headings
                    .iter()
                    .map(|heading| heading.slug.clone())
                    .collect(),
                block_ids: document.block_ids.clone(),
            },
        );
    }

    for document in documents.iter_mut() {
        for link in &mut document.links {
            let candidates = match link.kind {
                LinkKind::Markdown => {
                    resolve_markdown_link(&document.path, &link.target, &by_path, &by_path_lower)
                }
                LinkKind::Embed => {
                    if link.target.is_empty() && (link.anchor.is_some() || link.block_ref.is_some())
                    {
                        vec![document.path.clone()]
                    } else {
                        resolve_embed_link(
                            &document.path,
                            &link.target,
                            &by_path,
                            &by_path_lower,
                            &by_stem,
                        )
                    }
                }
                LinkKind::Wikilink => {
                    if link.target.is_empty() && (link.anchor.is_some() || link.block_ref.is_some())
                    {
                        vec![document.path.clone()]
                    } else {
                        resolve_wikilink(&link.target, &by_path, &by_path_lower, &by_stem)
                    }
                }
            };

            match candidates.as_slice() {
                [single] => {
                    link.resolved_path = Some(single.clone());
                    link.candidates = Vec::new();
                    validate_resolved_reference(link, single, &facts_by_path);
                }
                [] => {
                    link.status = LinkStatus::Unresolved;
                    link.resolved_path = None;
                    link.unresolved_reason = Some(UnresolvedReason::TargetMissing);
                    link.candidates = Vec::new();
                }
                many => {
                    link.status = LinkStatus::Ambiguous;
                    link.resolved_path = None;
                    link.unresolved_reason = Some(UnresolvedReason::Ambiguous);
                    link.candidates = many.to_vec();
                }
            }
        }
    }
}

#[derive(Clone)]
struct DocumentFacts {
    heading_slugs: Vec<String>,
    block_ids: Vec<String>,
}

fn validate_resolved_reference(
    link: &mut Link,
    target_path: &Utf8PathBuf,
    facts_by_path: &HashMap<Utf8PathBuf, DocumentFacts>,
) {
    let Some(facts) = facts_by_path.get(target_path) else {
        link.status = LinkStatus::Resolved;
        link.unresolved_reason = None;
        return;
    };

    if let Some(anchor) = &link.anchor {
        let anchor_slug = slugify(anchor);
        if !facts.heading_slugs.iter().any(|slug| slug == &anchor_slug) {
            link.status = LinkStatus::Unresolved;
            link.unresolved_reason = Some(UnresolvedReason::AnchorMissing);
            return;
        }
    }

    if let Some(block_ref) = &link.block_ref {
        if !facts.block_ids.iter().any(|block_id| block_id == block_ref) {
            link.status = LinkStatus::Unresolved;
            link.unresolved_reason = Some(UnresolvedReason::BlockRefMissing);
            return;
        }
    }

    link.status = LinkStatus::Resolved;
    link.unresolved_reason = None;
}

fn resolve_markdown_link(
    source_path: &Utf8Path,
    target: &str,
    by_path: &HashMap<String, Utf8PathBuf>,
    by_path_lower: &HashMap<String, Utf8PathBuf>,
) -> Vec<Utf8PathBuf> {
    let base = source_path.parent().unwrap_or_else(|| Utf8Path::new(""));
    resolve_path_like_target(base, target, by_path, by_path_lower)
}

fn resolve_embed_link(
    source_path: &Utf8Path,
    target: &str,
    by_path: &HashMap<String, Utf8PathBuf>,
    by_path_lower: &HashMap<String, Utf8PathBuf>,
    by_stem: &HashMap<String, Vec<Utf8PathBuf>>,
) -> Vec<Utf8PathBuf> {
    let base = source_path.parent().unwrap_or_else(|| Utf8Path::new(""));
    let base_matches = resolve_path_like_target(base, target, by_path, by_path_lower);
    if !base_matches.is_empty() {
        return base_matches;
    }

    let root_matches = resolve_path_like_target(Utf8Path::new(""), target, by_path, by_path_lower);
    if !root_matches.is_empty() {
        return root_matches;
    }

    resolve_wikilink(target, by_path, by_path_lower, by_stem)
}

fn resolve_wikilink(
    target: &str,
    by_path: &HashMap<String, Utf8PathBuf>,
    by_path_lower: &HashMap<String, Utf8PathBuf>,
    by_stem: &HashMap<String, Vec<Utf8PathBuf>>,
) -> Vec<Utf8PathBuf> {
    if target.contains('/') {
        let path_matches =
            resolve_path_like_target(Utf8Path::new(""), target, by_path, by_path_lower);
        if !path_matches.is_empty() {
            return path_matches;
        }
    }

    let stem = Utf8Path::new(target).file_stem().unwrap_or(target);
    by_stem
        .get(&stem.to_lowercase())
        .cloned()
        .unwrap_or_default()
}

fn split_anchor(raw: &str) -> (String, Option<String>) {
    match raw.split_once('#') {
        Some((target, anchor)) => (target.to_string(), Some(anchor.to_string())),
        None => (raw.to_string(), None),
    }
}

fn split_anchor_or_block_ref(raw: &str) -> (String, Option<String>, Option<String>) {
    match raw.split_once('#') {
        Some((target, reference)) if reference.starts_with('^') => {
            (target.to_string(), None, Some(reference[1..].to_string()))
        }
        Some((target, anchor)) => (target.to_string(), Some(anchor.to_string()), None),
        None => (raw.to_string(), None, None),
    }
}

fn source_span(content: &str, byte_offset: usize) -> SourceSpan {
    let prefix = &content[..byte_offset.min(content.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len() + 1, |(_, tail)| tail.len() + 1);

    SourceSpan {
        line,
        column,
        byte_offset,
    }
}

fn is_local_markdown_target(target: &str) -> bool {
    if !is_local_file_target(target) {
        return false;
    }

    let (target, _) = split_anchor(target);
    !target.is_empty()
}

fn is_local_file_target(target: &str) -> bool {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with('#')
    {
        return false;
    }

    true
}

fn decode_percent_escapes(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                output.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn resolve_path_like_target(
    base: &Utf8Path,
    target: &str,
    by_path: &HashMap<String, Utf8PathBuf>,
    by_path_lower: &HashMap<String, Utf8PathBuf>,
) -> Vec<Utf8PathBuf> {
    let candidate = normalize_relative(base, target);
    if let Some(path) = by_path.get(candidate.as_str()) {
        return vec![path.clone()];
    }
    if let Some(path) = by_path_lower.get(&candidate.as_str().to_lowercase()) {
        return vec![path.clone()];
    }

    if candidate.extension().is_none() {
        let with_markdown_extension = candidate.with_extension("md");
        if let Some(path) = by_path.get(with_markdown_extension.as_str()) {
            return vec![path.clone()];
        }
        if let Some(path) = by_path_lower.get(&with_markdown_extension.as_str().to_lowercase()) {
            return vec![path.clone()];
        }
    }

    Vec::new()
}

fn normalize_relative(base: &Utf8Path, target: &str) -> Utf8PathBuf {
    let joined = base.join(target);
    let mut normalized = Utf8PathBuf::new();
    for component in joined.as_std_path().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part.to_string_lossy().as_ref()),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    normalized
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_end_matches('-').to_string()
}
