//! Session browser model: the summary shown for one Pi session file (title,
//! search match, ordering), Pi's per-cwd session directory name, and the
//! timestamp parsing/formatting the summaries use.

use std::path::{Path, PathBuf};

use crate::i18n::{Language, tr};

/// What the browser shows for one session file.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionSummary {
    pub path: PathBuf,
    pub id: String,
    /// The header's `cwd`; empty when the header has none.
    pub cwd: PathBuf,
    /// Latest `session_info` name (`/name`, `set_session_name`); a later empty
    /// name clears it, as in Pi.
    pub name: Option<String>,
    /// Text of the first user message.
    pub first_message: Option<String>,
    /// Last user/assistant activity, else header time, else file mtime (ms).
    pub modified_ms: i64,
    /// Number of `message` entries (Pi counts every role).
    pub message_count: usize,
    /// The scan stopped at [`MAX_SCAN_BYTES`]; counts are a lower bound.
    pub truncated: bool,
    /// Model of the last assistant reply or `model_change`, whichever is later.
    pub model: Option<String>,
}

impl SessionSummary {
    /// The title: the session's name, or what the first prompt said, without
    /// the `Attached files:` block it carried (a prompt of files only is
    /// titled by their names).
    pub fn title(&self) -> String {
        self.title_in(Language::English)
    }

    /// The same title with app-generated fallback text in `language`.
    pub fn title_in(&self, language: Language) -> String {
        let text = match (self.name.as_deref(), self.first_message.as_deref()) {
            (Some(name), _) => name.to_string(),
            (None, Some(first)) => crate::attachments::summary(first),
            (None, None) => tr(language, "(no messages)").to_string(),
        };
        crate::text::first_line(text.trim(), 120)
    }

    pub fn matches(&self, query_lower: &str) -> bool {
        query_lower.is_empty()
            || [self.name.as_deref(), self.first_message.as_deref()]
                .into_iter()
                .flatten()
                .any(|t| t.to_lowercase().contains(query_lower))
            || self
                .cwd
                .to_string_lossy()
                .to_lowercase()
                .contains(query_lower)
            || self
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(query_lower)
    }
}

/// Stop reading a session after this many bytes; huge transcripts (inline
/// images, long tool output) would otherwise stall the list.
pub const MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024;

/// Pi's per-cwd directory name: drop the leading separator, map `/ \ :` to `-`.
pub fn session_dir_name(cwd: &Path) -> String {
    let s = cwd.to_string_lossy();
    let trimmed = s.strip_prefix(['/', '\\']).unwrap_or(&s);
    let safe: String = trimmed
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') {
                '-'
            } else {
                c
            }
        })
        .collect();
    format!("--{safe}--")
}

pub fn sort_newest_first(sessions: &mut [SessionSummary]) {
    sessions.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| b.path.cmp(&a.path))
    });
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY-MM-DDTHH:MM:SS[.fff][Z|±HH:MM]` to Unix milliseconds.
pub fn parse_iso_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d, h, mi, sec) = (
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
        num(17..19)?,
    );
    let mut i = 19;
    let mut ms = 0;
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let frac = s.get(start..i)?;
        let padded: String = frac.chars().chain("000".chars()).take(3).collect();
        ms = padded.parse::<i64>().ok()?;
    }
    let offset_min = match b.get(i) {
        None | Some(b'Z' | b'z') => 0,
        Some(sign @ (b'+' | b'-')) => {
            let oh = num(i + 1..i + 3)?;
            let om = match b.get(i + 3) {
                Some(b':') => num(i + 4..i + 6)?,
                _ => num(i + 3..i + 5).unwrap_or(0),
            };
            let total = oh * 60 + om;
            if *sign == b'+' { total } else { -total }
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo, d);
    Some(((days * 86_400 + h * 3600 + mi * 60 + sec - offset_min * 60) * 1000) + ms)
}

pub fn relative_time(now_ms: i64, then_ms: i64) -> String {
    relative_time_in(Language::English, now_ms, then_ms)
}

pub fn relative_time_in(language: Language, now_ms: i64, then_ms: i64) -> String {
    let secs = (now_ms - then_ms).max(0) / 1000;
    let (key, unit) = match secs {
        0..60 => return tr(language, "just now").to_string(),
        60..3600 => ("{count}m ago", 60),
        3600..86_400 => ("{count}h ago", 3600),
        86_400..604_800 => ("{count}d ago", 86_400),
        604_800..2_592_000 => ("{count}w ago", 604_800),
        2_592_000..5_184_000 => return tr(language, "1mo ago").to_string(),
        5_184_000..31_536_000 => ("{count}mo ago", 2_592_000),
        31_536_000..63_072_000 => return tr(language, "1y ago").to_string(),
        _ => ("{count}y ago", 31_536_000),
    };
    tr(language, key).replace("{count}", &(secs / unit).to_string())
}

/// The title of a session from Pi's own listing (Gentle Shell's
/// `list_sessions`): its name, else its first message, on one line and
/// without the `Attached files:` block the prompt carried.
pub fn listed_title(name: Option<&str>, first_message: &str) -> String {
    let text = name.map_or_else(
        || crate::attachments::summary(first_message),
        str::to_string,
    );
    crate::text::first_line(text.trim(), 120)
}

/// Whether a session from Pi's own listing matches a lowercase query: by
/// name, first message or project name (the listing carries no path).
pub fn listed_matches(
    query_lower: &str,
    name: Option<&str>,
    first_message: &str,
    project: &str,
) -> bool {
    query_lower.is_empty()
        || [name, Some(first_message), Some(project)]
            .into_iter()
            .flatten()
            .any(|t| t.to_lowercase().contains(query_lower))
}

/// How Pi's listing names `cwd`'s project: its last segment, or `unknown`.
pub fn project_name(cwd: &Path) -> String {
    cwd.file_name().map_or_else(
        || "unknown".to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The session id in a Pi session file name (`<timestamp>_<id>.jsonl`).
pub fn file_session_id(path: &Path) -> Option<&str> {
    let stem = path.file_stem()?.to_str()?;
    let (_, id) = stem.rsplit_once('_')?;
    (!id.is_empty()).then_some(id)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        SessionSummary, file_session_id, listed_matches, listed_title, parse_iso_ms, project_name,
        relative_time, session_dir_name,
    };

    /// A session whose only content is its first message.
    fn session(first_message: Option<&str>) -> SessionSummary {
        SessionSummary {
            path: PathBuf::from("/s/--w--/a.jsonl"),
            id: "a".to_string(),
            cwd: PathBuf::from("/w"),
            name: None,
            first_message: first_message.map(str::to_string),
            modified_ms: 0,
            message_count: 1,
            truncated: false,
            model: None,
        }
    }

    /// S9: a session's title is what the person typed, never the
    /// `Attached files:` block its prompt carried; a prompt that was only
    /// files is titled by their names (Pi's own listing too).
    #[test]
    fn session_titles_hide_the_attached_files_block() {
        let block = "\n\nAttached files:\n- src/main.rs\n- /work/report.pdf";
        assert_eq!(
            session(Some(&format!("Fix the build{block}"))).title(),
            "Fix the build"
        );
        assert_eq!(
            session(Some("Attached files:\n- report.pdf\n- /work/notes.txt")).title(),
            "report.pdf, notes.txt"
        );
        assert_eq!(session(None).title(), "(no messages)");
        assert_eq!(
            listed_title(None, &format!("Fix the build{block}")),
            "Fix the build"
        );
        assert_eq!(
            listed_title(None, "Attached files:\n- report.pdf"),
            "report.pdf"
        );
        assert_eq!(
            listed_title(Some("Release prep"), "Attached files:\n- a.pdf"),
            "Release prep"
        );
    }

    #[test]
    fn listed_sessions_title_and_match_without_paths() {
        assert_eq!(
            listed_title(Some("Release prep"), "Prepare"),
            "Release prep"
        );
        assert_eq!(listed_title(None, "  Why\nmore"), "Why …");
        let matches = |q: &str| listed_matches(q, Some("Release prep"), "Notes", "gentle-shell");
        assert!(matches(""));
        assert!(matches("release"));
        assert!(matches("notes"));
        assert!(matches("gentle-sh"));
        assert!(!matches("windows"));
        assert!(listed_matches("why", None, "Why does it fail", "x"));
    }

    #[test]
    fn project_names_follow_pi() {
        assert_eq!(project_name(Path::new("/w/gentle-shell")), "gentle-shell");
        assert_eq!(project_name(Path::new("/")), "unknown");
    }

    #[test]
    fn file_names_carry_the_session_id() {
        let path = Path::new("/s/--w--/2026-11-17T10-00-00-000Z_5e21f0aa-3b7c.jsonl");
        assert_eq!(file_session_id(path), Some("5e21f0aa-3b7c"));
        assert_eq!(file_session_id(Path::new("/s/plain.jsonl")), None);
    }

    #[test]
    fn encodes_cwd_like_pi() {
        assert_eq!(
            session_dir_name(Path::new(
                "/Users/ismaelosuna/Documents/Git/personal/gentle-shell-gpui"
            )),
            "--Users-ismaelosuna-Documents-Git-personal-gentle-shell-gpui--"
        );
        assert_eq!(session_dir_name(Path::new("C:\\work\\x")), "--C--work-x--");
    }

    #[test]
    fn parses_iso_timestamps() {
        assert_eq!(
            parse_iso_ms("2024-12-03T14:00:00.000Z"),
            Some(1_733_234_400_000)
        );
        assert_eq!(
            parse_iso_ms("2024-12-03T14:00:00Z"),
            Some(1_733_234_400_000)
        );
        assert_eq!(
            parse_iso_ms("2024-12-03T14:00:00.5Z"),
            Some(1_733_234_400_500)
        );
        assert_eq!(
            parse_iso_ms("2024-12-03T16:00:00+02:00"),
            Some(1_733_234_400_000)
        );
        assert_eq!(parse_iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso_ms("yesterday"), None);
    }

    #[test]
    fn formats_relative_time() {
        let now = 1_000_000_000_000;
        assert_eq!(relative_time(now, now + 5_000), "just now");
        assert_eq!(relative_time(now, now - 30_000), "just now");
        assert_eq!(relative_time(now, now - 5 * 60_000), "5m ago");
        assert_eq!(relative_time(now, now - 3 * 3_600_000), "3h ago");
        assert_eq!(relative_time(now, now - 2 * 86_400_000), "2d ago");
        assert_eq!(relative_time(now, now - 14 * 86_400_000), "2w ago");
        assert_eq!(relative_time(now, now - 400 * 86_400_000), "1y ago");
    }
}
