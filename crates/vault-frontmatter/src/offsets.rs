use std::ops::Range;

pub struct FrontmatterPropertyString<'a> {
    pub property: String,
    pub text: &'a str,
    pub offset: Option<usize>,
}

pub fn frontmatter_property_strings<'a>(
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

pub fn frontmatter_scalar_offset(
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

pub fn frontmatter_list_item_offset(
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
