use chrono::{Datelike, NaiveDateTime, Timelike};

/// Render a NaiveDateTime through a Moment-subset format string.
///
/// Supported tokens (v1):
/// - Year: YYYY, YY
/// - Month: MM, M, MMM, MMMM
/// - Day: DD, D
/// - Hour: HH, H, hh, h
/// - Minute: mm
/// - Second: ss
/// - AM/PM: A, a
/// - Day of week: dddd, ddd
///
/// Bracket escape: `[X]` renders X as literal (where X may be a token letter).
/// Unsupported token-shaped sequences render as themselves.
pub fn format_datetime(fmt: &str, t: &NaiveDateTime) -> String {
    let mut out = String::with_capacity(fmt.len() + 8);
    let bytes = fmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(end) = fmt[i + 1..].find(']') {
                out.push_str(&fmt[i + 1..i + 1 + end]);
                i += end + 2;
                continue;
            }
            out.push('[');
            i += 1;
            continue;
        }
        let remaining = &fmt[i..];
        if let Some((tok, rendered)) = try_token(remaining, t) {
            out.push_str(&rendered);
            i += tok.len();
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn try_token(s: &str, t: &NaiveDateTime) -> Option<(&'static str, String)> {
    const TOKENS: &[&str] = &[
        "MMMM", "dddd", "YYYY", "MMM", "ddd", "YY", "MM", "DD", "HH", "hh", "mm", "ss", "M", "D",
        "H", "h", "A", "a",
    ];
    for tok in TOKENS {
        if s.starts_with(tok) {
            return Some((tok, render_token(tok, t)));
        }
    }
    None
}

fn render_token(tok: &str, t: &NaiveDateTime) -> String {
    match tok {
        "YYYY" => format!("{:04}", t.year()),
        "YY" => format!("{:02}", t.year() % 100),
        "MM" => format!("{:02}", t.month()),
        "M" => format!("{}", t.month()),
        "MMM" => MONTH_ABBR[(t.month() - 1) as usize].to_string(),
        "MMMM" => MONTH_FULL[(t.month() - 1) as usize].to_string(),
        "DD" => format!("{:02}", t.day()),
        "D" => format!("{}", t.day()),
        "HH" => format!("{:02}", t.hour()),
        "H" => format!("{}", t.hour()),
        "hh" => format!("{:02}", hour_12(t.hour())),
        "h" => format!("{}", hour_12(t.hour())),
        "mm" => format!("{:02}", t.minute()),
        "ss" => format!("{:02}", t.second()),
        "A" => (if t.hour() < 12 { "AM" } else { "PM" }).to_string(),
        "a" => (if t.hour() < 12 { "am" } else { "pm" }).to_string(),
        "ddd" => DOW_ABBR[t.weekday().num_days_from_monday() as usize].to_string(),
        "dddd" => DOW_FULL[t.weekday().num_days_from_monday() as usize].to_string(),
        _ => unreachable!("try_token only returns known tokens"),
    }
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
    }

    #[test]
    fn formats_hour_minute_tokens() {
        let t = dt(2026, 5, 25, 18, 30, 45);
        assert_eq!(format_datetime("HH:mm:ss", &t), "18:30:45");
        assert_eq!(format_datetime("h:mm A", &t), "6:30 PM");
        assert_eq!(format_datetime("hh a", &t), "06 pm");
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
}
