//! Render a NaiveDateTime through a Moment-subset format string.
//!
//! Supported tokens (v1):
//! - Year: YYYY, YY
//! - Month: MM, M, MMM, MMMM
//! - Day: DD, D
//! - Hour: HH, H, hh, h
//! - Minute: mm
//! - Second: ss
//! - AM/PM: A, a
//! - Day of week: dddd, ddd
//!
//! Bracket escape: `[X]` renders X as literal (where X may be a token letter).
//! An unmatched `[` (no closing `]`) is emitted as a literal `[` and scanning continues.
//! Unsupported token-shaped sequences render as themselves.
//!
//! This module is the format-token layer of the substitution engine; variable
//! resolution (`{{var}}`) and pipe transforms (`{{var | transform}}`) are layered
//! in Tasks 1.2/1.3 of the vault-new arc.

use chrono::{Datelike, NaiveDateTime, Timelike};

/// Known format tokens.
///
/// Variants are declared longest-prefix-first within each family so that a
/// linear scan of `Token::ALL` correctly applies longest-match. When adding a
/// new token that shares a leading prefix with an existing one, insert it
/// *before* the shorter sibling (e.g. MMMM before MMM before MM before M).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    /// `MMMM` — full month name
    Mmmm,
    /// `dddd` — full weekday name
    Dddd,
    /// `YYYY` — 4-digit year
    Yyyy,
    /// `MMM` — abbreviated month name
    Mmm,
    /// `ddd` — abbreviated weekday name
    Ddd,
    /// `YY` — 2-digit year
    Yy,
    /// `MM` — zero-padded month
    Mm,
    /// `DD` — zero-padded day
    Dd,
    /// `HH` — zero-padded 24-hour
    Hh24,
    /// `hh` — zero-padded 12-hour
    Hh12,
    /// `mm` — zero-padded minute
    MmMinute,
    /// `ss` — zero-padded second
    Ss,
    /// `M` — bare month
    M,
    /// `D` — bare day
    D,
    /// `H` — bare 24-hour
    H24,
    /// `h` — bare 12-hour
    H12,
    /// `A` — uppercase AM/PM
    AUpper,
    /// `a` — lowercase am/pm
    ALower,
}

impl Token {
    /// All tokens in longest-match-first order.
    const ALL: &'static [(Token, &'static str)] = &[
        (Token::Mmmm, "MMMM"),
        (Token::Dddd, "dddd"),
        (Token::Yyyy, "YYYY"),
        (Token::Mmm, "MMM"),
        (Token::Ddd, "ddd"),
        (Token::Yy, "YY"),
        (Token::Mm, "MM"),
        (Token::Dd, "DD"),
        (Token::Hh24, "HH"),
        (Token::Hh12, "hh"),
        (Token::MmMinute, "mm"),
        (Token::Ss, "ss"),
        (Token::M, "M"),
        (Token::D, "D"),
        (Token::H24, "H"),
        (Token::H12, "h"),
        (Token::AUpper, "A"),
        (Token::ALower, "a"),
    ];

    fn render(self, t: &NaiveDateTime) -> String {
        match self {
            Token::Yyyy => format!("{:04}", t.year()),
            Token::Yy => format!("{:02}", t.year() % 100),
            Token::Mm => format!("{:02}", t.month()),
            Token::M => format!("{}", t.month()),
            Token::Mmm => MONTH_ABBR[(t.month() - 1) as usize].to_string(),
            Token::Mmmm => MONTH_FULL[(t.month() - 1) as usize].to_string(),
            Token::Dd => format!("{:02}", t.day()),
            Token::D => format!("{}", t.day()),
            Token::Hh24 => format!("{:02}", t.hour()),
            Token::H24 => format!("{}", t.hour()),
            Token::Hh12 => format!("{:02}", hour_12(t.hour())),
            Token::H12 => format!("{}", hour_12(t.hour())),
            Token::MmMinute => format!("{:02}", t.minute()),
            Token::Ss => format!("{:02}", t.second()),
            Token::AUpper => (if t.hour() < 12 { "AM" } else { "PM" }).to_string(),
            Token::ALower => (if t.hour() < 12 { "am" } else { "pm" }).to_string(),
            Token::Ddd => DOW_ABBR[t.weekday().num_days_from_monday() as usize].to_string(),
            Token::Dddd => DOW_FULL[t.weekday().num_days_from_monday() as usize].to_string(),
        }
    }
}

/// Render a [`NaiveDateTime`] through a Moment-subset format string.
pub fn format_datetime(fmt: &str, t: &NaiveDateTime) -> String {
    let mut out = String::with_capacity(fmt.len() + 8);
    let bytes = fmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Bracket-escape: `[` and `]` are ASCII, so byte indexing is safe here.
        if bytes[i] == b'[' {
            if let Some(end) = fmt[i + 1..].find(']') {
                out.push_str(&fmt[i + 1..i + 1 + end]);
                i += end + 2;
                continue;
            }
            // Unmatched `[`: emit literal and continue.
            out.push('[');
            i += 1;
            continue;
        }
        // Try to match a known token at this position.
        let remaining = &fmt[i..];
        if let Some((tok, src_len)) = try_token(remaining, t) {
            out.push_str(&tok);
            i += src_len;
        } else {
            // Fallthrough: emit the next Unicode scalar value as a literal.
            // `remaining` is a valid &str, so chars() is safe and advances by
            // the correct number of bytes for multi-byte codepoints.
            let ch = remaining.chars().next().expect("non-empty remaining");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Try to match a format token at the start of `s`.
///
/// Returns `(rendered_output, source_byte_length)` on success.
fn try_token(s: &str, t: &NaiveDateTime) -> Option<(String, usize)> {
    for &(token, pattern) in Token::ALL {
        if s.starts_with(pattern) {
            return Some((token.render(t), pattern.len()));
        }
    }
    None
}

fn hour_12(h: u32) -> u32 {
    let h = h % 12;
    if h == 0 {
        12
    } else {
        h
    }
}

const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTH_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DOW_ABBR: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DOW_FULL: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

#[cfg(test)]
mod format_tests {
    use super::format_datetime;
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_time(NaiveTime::from_hms_opt(h, mi, s).unwrap())
    }

    #[test]
    fn formats_year_tokens() {
        let t = dt(2026, 5, 25, 18, 30, 0);
        assert_eq!(format_datetime("YYYY", &t), "2026");
        assert_eq!(format_datetime("YY", &t), "26");
    }

    #[test]
    fn formats_month_day_tokens() {
        let t = dt(2026, 5, 25, 18, 30, 0);
        assert_eq!(format_datetime("YYYY-MM-DD", &t), "2026-05-25");
        assert_eq!(format_datetime("M/D", &t), "5/25");
        assert_eq!(format_datetime("MMM D", &t), "May 25");
        assert_eq!(format_datetime("MMMM D", &t), "May 25");
        // January: short and long forms differ, catching any MMM/MMMM routing bug.
        let jan = dt(2026, 1, 15, 12, 0, 0);
        assert_eq!(format_datetime("MMM", &jan), "Jan");
        assert_eq!(format_datetime("MMMM", &jan), "January");
    }

    #[test]
    fn formats_hour_minute_tokens() {
        let t = dt(2026, 5, 25, 18, 30, 45);
        assert_eq!(format_datetime("HH:mm:ss", &t), "18:30:45");
        assert_eq!(format_datetime("h:mm A", &t), "6:30 PM");
        assert_eq!(format_datetime("hh a", &t), "06 pm");
        // Midnight: h=0 → 12 AM
        let midnight = dt(2026, 5, 25, 0, 0, 0);
        assert_eq!(format_datetime("h:mm A", &midnight), "12:00 AM");
        // Noon: h=12 → 12 PM
        let noon = dt(2026, 5, 25, 12, 0, 0);
        assert_eq!(format_datetime("h:mm A", &noon), "12:00 PM");
        assert_eq!(format_datetime("hh a", &noon), "12 pm");
    }

    #[test]
    fn formats_day_of_week_tokens() {
        let t = dt(2026, 5, 25, 18, 30, 0); // Monday
        assert_eq!(format_datetime("ddd", &t), "Mon");
        assert_eq!(format_datetime("dddd", &t), "Monday");
    }

    #[test]
    fn literals_pass_through() {
        let t = dt(2026, 5, 25, 18, 30, 0);
        assert_eq!(
            format_datetime("YYYY-MM-DD'T'HH:mm", &t),
            "2026-05-25'T'18:30"
        );
        assert_eq!(format_datetime("YYYY-MM-DDTHH:mm", &t), "2026-05-25T18:30");
    }

    #[test]
    fn bracket_escape_for_token_letters() {
        let t = dt(2026, 5, 25, 18, 30, 0);
        assert_eq!(format_datetime("[Year] YYYY", &t), "Year 2026");
    }

    #[test]
    fn unsupported_tokens_render_as_literal() {
        let t = dt(2026, 5, 25, 18, 30, 0);
        assert_eq!(format_datetime("Q", &t), "Q");
    }

    #[test]
    fn non_ascii_literals_pass_through() {
        let t = dt(2026, 5, 25, 18, 30, 0);
        // CJK separators: each kanji is 3 bytes; must not panic on byte-boundary.
        assert_eq!(format_datetime("YYYY年MM月DD日", &t), "2026年05月25日");
        // En-dash typographic separator (3 bytes in UTF-8).
        assert_eq!(format_datetime("YYYY–MM–DD", &t), "2026–05–25");
        // Bracket-escaped CJK literal.
        assert_eq!(format_datetime("[今日] YYYY-MM-DD", &t), "今日 2026-05-25");
    }
}
