use std::ops::Range;

use vault_core::Diagnostic;

pub(crate) struct FrontmatterPropertyString<'a> {
    pub property: String,
    pub text: &'a str,
    pub offset: Option<usize>,
}

pub(crate) fn extract_frontmatter<'a>(
    content: &'a str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    Option<serde_json::Value>,
    Option<Range<usize>>,
    &'a str,
    usize,
) {
    let Some(after_open) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return (None, None, content, 0);
    };

    let mut offset = content.len() - after_open.len();
    let yaml_start = offset;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            let yaml = &content[yaml_start..offset];
            let yaml_range = yaml_start..offset;
            let body_start = offset + line.len();
            let body = &content[body_start..];
            return match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
                Ok(value) => match serde_json::to_value(value) {
                    Ok(value) => (Some(value), Some(yaml_range), body, body_start),
                    Err(error) => {
                        diagnostics.push(
                            Diagnostic::warning(
                                "frontmatter-json-conversion-failed",
                                "frontmatter could not be converted to JSON",
                            )
                            .with_detail(error.to_string()),
                        );
                        (None, Some(yaml_range), body, body_start)
                    }
                },
                Err(error) => {
                    diagnostics.push(
                        Diagnostic::warning(
                            "frontmatter-parse-failed",
                            "frontmatter could not be parsed",
                        )
                        .with_detail(error.to_string()),
                    );
                    (None, Some(yaml_range), body, body_start)
                }
            };
        }
        offset += line.len();
    }

    diagnostics.push(Diagnostic::warning(
        "frontmatter-unclosed",
        "frontmatter opening delimiter has no closing delimiter",
    ));
    (None, None, content, 0)
}

pub(crate) fn frontmatter_property_strings<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    content: &str,
    frontmatter_range: Option<Range<usize>>,
) -> Vec<FrontmatterPropertyString<'a>> {
    let mut strings = Vec::new();

    for (property, value) in object {
        match value {
            serde_json::Value::String(text) => strings.push(FrontmatterPropertyString {
                property: property.to_string(),
                text,
                offset: frontmatter_scalar_offset(
                    content,
                    frontmatter_range.clone(),
                    property,
                    text,
                ),
            }),
            serde_json::Value::Array(values) => {
                for text in values.iter().filter_map(|value| value.as_str()) {
                    strings.push(FrontmatterPropertyString {
                        property: property.to_string(),
                        text,
                        offset: frontmatter_list_item_offset(
                            content,
                            frontmatter_range.clone(),
                            property,
                            text,
                        ),
                    });
                }
            }
            _ => {}
        }
    }

    strings
}

fn frontmatter_scalar_offset(
    content: &str,
    frontmatter_range: Option<Range<usize>>,
    property: &str,
    text: &str,
) -> Option<usize> {
    let range = frontmatter_range?;
    let yaml = &content[range.clone()];
    let property_prefix = format!("{property}:");
    let mut line_start = range.start;

    for line in yaml.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !line.starts_with([' ', '\t']) && trimmed.starts_with(&property_prefix) {
            return line.find(text).map(|offset| line_start + offset);
        }
        line_start += line.len();
    }

    None
}

fn frontmatter_list_item_offset(
    content: &str,
    frontmatter_range: Option<Range<usize>>,
    property: &str,
    text: &str,
) -> Option<usize> {
    let range = frontmatter_range?;
    let yaml = &content[range.clone()];
    let property_prefix = format!("{property}:");
    let mut in_property = false;
    let mut line_start = range.start;

    for line in yaml.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !line.starts_with([' ', '\t']) {
            in_property = trimmed.starts_with(&property_prefix);
            line_start += line.len();
            continue;
        }

        if in_property && trimmed.starts_with('-') {
            if let Some(offset) = line.find(text) {
                return Some(line_start + offset);
            }
        }

        line_start += line.len();
    }

    None
}
