use std::collections::HashMap;

use serde_json::Value;
use vault_core::Document;

pub(crate) fn frontmatter_value_matches(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::String(actual), Value::String(expected)) => actual == expected,
        (Value::Bool(actual), Value::Bool(expected)) => actual == expected,
        (Value::Number(actual), Value::Number(expected)) => actual == expected,
        _ => false,
    }
}

pub(crate) fn frontmatter_type_matches(value: &Value, expected_type: &str) -> bool {
    match expected_type {
        "datetime" => value
            .as_str()
            .is_some_and(|value| is_datetime_string(value)),
        "date" => value.as_str().is_some_and(is_date_string),
        "list_of_strings" => value
            .as_array()
            .is_some_and(|values| values.iter().all(|value| value.as_str().is_some())),
        "wikilink" => value.as_str().is_some_and(is_wikilink_string),
        "wikilink_or_list" => {
            value.as_str().is_some_and(is_wikilink_string)
                || value.as_array().is_some_and(|values| {
                    values
                        .iter()
                        .all(|value| value.as_str().is_some_and(is_wikilink_string))
                })
        }
        _ => false,
    }
}

pub(crate) fn is_datetime_string(value: &str) -> bool {
    if value.len() < 16 {
        return false;
    }

    let Some((date, time)) = value.split_once('T').or_else(|| value.split_once(' ')) else {
        return false;
    };

    is_date_string(date) && is_time_string(time)
}

pub(crate) fn is_date_string(value: &str) -> bool {
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };

    year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && year.chars().all(|char| char.is_ascii_digit())
        && month
            .parse::<u8>()
            .is_ok_and(|month| (1..=12).contains(&month))
        && day.parse::<u8>().is_ok_and(|day| (1..=31).contains(&day))
}

pub(crate) fn is_time_string(value: &str) -> bool {
    let time = value
        .strip_suffix('Z')
        .unwrap_or(value)
        .split_once(['+', '-'])
        .map_or(value, |(time, _)| time);
    let mut parts = time.split(':');
    let (Some(hour), Some(minute), maybe_second, None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };

    hour.len() == 2
        && minute.len() == 2
        && hour.parse::<u8>().is_ok_and(|hour| hour <= 23)
        && minute.parse::<u8>().is_ok_and(|minute| minute <= 59)
        && maybe_second.is_none_or(|second| {
            second.len() == 2 && second.parse::<u8>().is_ok_and(|second| second <= 59)
        })
}

pub(crate) fn is_wikilink_string(value: &str) -> bool {
    value.starts_with("[[") && value.ends_with("]]") && value.len() > 4
}

pub(crate) fn document_has_frontmatter_field(document: &Document, field: &str) -> bool {
    document_frontmatter_field(document, field).is_some()
}

pub(crate) fn document_frontmatter_field<'a>(
    document: &'a Document,
    field: &str,
) -> Option<&'a Value> {
    document
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| frontmatter.get(field))
        .filter(|value| !value.is_null())
}

pub(crate) fn frontmatter_predicates_match(
    document: &Document,
    predicates: &HashMap<String, Value>,
) -> bool {
    if predicates.is_empty() {
        return true;
    }

    let Some(frontmatter) = document.frontmatter.as_ref() else {
        return false;
    };

    predicates.iter().all(|(field, expected)| {
        frontmatter
            .get(field)
            .is_some_and(|actual| frontmatter_value_matches(actual, expected))
    })
}
