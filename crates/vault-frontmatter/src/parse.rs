use std::ops::Range;

use vault_core::Diagnostic;

pub fn extract_frontmatter<'a>(
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
