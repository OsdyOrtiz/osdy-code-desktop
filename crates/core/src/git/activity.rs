//! The Git pane's activity strip: a year of activity as a grid of days, a
//! column per week (Sunday first), each day shaded by how much happened,
//! like GitHub's contribution graph. Two sources: this repository's
//! commits, or the user's GitHub contribution calendar (every repository,
//! pull requests, issues and reviews), read from their public profile page.

use crate::i18n::{Language, tr};
use crate::usage::time::{DAY, civil_from_days, days_from_civil};

/// Weeks the grid spans: a year, plus the week in progress.
pub const WEEKS: usize = 53;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// The `git log` arguments that print what [`parse_times`] reads: every
/// ref's commits but merges, by author time.
pub const LOG_ARGS: &[&str] = &["log", "--all", "--no-merges", "--format=%at"];

/// [`LOG_ARGS`]' output: Unix seconds, one per commit.
pub fn parse_times(raw: &str) -> Vec<i64> {
    raw.lines().filter_map(|l| l.trim().parse().ok()).collect()
}

/// The local day (days since 1970-01-01) of Unix time `secs`.
pub fn local_day(secs: i64, offset: i64) -> i64 {
    (secs + offset).div_euclid(DAY)
}

/// Commits per local day.
pub fn count_days(times: &[i64], offset: i64) -> Vec<(i64, u32)> {
    let mut days: Vec<i64> = times.iter().map(|t| local_day(*t, offset)).collect();
    days.sort_unstable();
    let mut counts: Vec<(i64, u32)> = Vec::new();
    for day in days {
        match counts.last_mut() {
            Some((last, n)) if *last == day => *n += 1,
            _ => counts.push((day, 1)),
        }
    }
    counts
}

/// What the strip counts, for its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Commits,
    Contributions,
}

impl Unit {
    fn word(self, language: Language, n: u32) -> &'static str {
        tr(
            language,
            match (self, n) {
                (Unit::Commits, 1) => "commit",
                (Unit::Commits, _) => "commits",
                (Unit::Contributions, 1) => "contribution",
                (Unit::Contributions, _) => "contributions",
            },
        )
    }
}

/// One cell of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    /// Local days since 1970-01-01.
    pub day: i64,
    pub count: u32,
    /// 0 (none) to 4 (the busiest days), for the shade.
    pub level: u8,
    /// After today: the cell isn't drawn.
    pub future: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// Oldest first; each week Sunday to Saturday. The last holds today.
    pub weeks: Vec<[Day; 7]>,
}

impl Activity {
    /// The last `weeks` weeks (all of them when fewer are kept), for a pane
    /// too narrow for the year.
    pub fn last(&self, weeks: usize) -> &[[Day; 7]] {
        &self.weeks[self.weeks.len().saturating_sub(weeks)..]
    }
}

/// The total of `weeks`.
pub fn total(weeks: &[[Day; 7]]) -> u32 {
    weeks.iter().flatten().map(|d| d.count).sum()
}

/// The grid ending with `today`'s week from counts per day (any order;
/// days outside the grid are left out).
pub fn activity(counts: &[(i64, u32)], today: i64) -> Activity {
    let start = today - weekday(today) as i64 - (WEEKS as i64 - 1) * 7;
    let mut cells = vec![0u32; WEEKS * 7];
    for (day, count) in counts {
        if (start..=today).contains(day) {
            cells[(day - start) as usize] += count;
        }
    }
    let shades = Shades::new(&cells);
    let weeks = (0..WEEKS)
        .map(|w| {
            std::array::from_fn(|d| {
                let index = w * 7 + d;
                let day = start + index as i64;
                Day {
                    day,
                    count: cells[index],
                    level: shades.level(cells[index]),
                    future: day > today,
                }
            })
        })
        .collect();
    Activity { weeks }
}

/// Where the shades change: the quartiles of the days with activity, as
/// GitHub shades its graph, so a few very busy days don't wash every other
/// day out to the palest shade.
struct Shades([u32; 3]);

impl Shades {
    fn new(cells: &[u32]) -> Self {
        let mut busy: Vec<u32> = cells.iter().copied().filter(|c| *c > 0).collect();
        busy.sort_unstable();
        let at = |p: f32| {
            busy.get(((busy.len().saturating_sub(1)) as f32 * p).round() as usize)
                .copied()
                .unwrap_or(0)
        };
        Shades([at(0.25), at(0.5), at(0.75)])
    }

    fn level(&self, count: u32) -> u8 {
        match count {
            0 => 0,
            c if c <= self.0[0] => 1,
            c if c <= self.0[1] => 2,
            c if c <= self.0[2] => 3,
            _ => 4,
        }
    }
}

/// 0 for Sunday.
pub fn weekday(day: i64) -> usize {
    // 1970-01-01 was a Thursday.
    (day + 4).rem_euclid(7) as usize
}

/// Month names over the columns of `weeks`: `(column, "Jan")` where a
/// month's first Sunday falls, without one crowding the next.
pub fn month_labels(language: Language, weeks: &[[Day; 7]]) -> Vec<(usize, &'static str)> {
    let month = |day: i64| civil_from_days(day).1;
    let mut labels: Vec<(usize, &'static str)> = Vec::new();
    for (column, week) in weeks.iter().enumerate() {
        let first = week[0].day;
        if column == 0 || month(first) != month(weeks[column - 1][0].day) {
            labels.push((column, tr(language, MONTHS[month(first) as usize - 1])));
        }
    }
    // A label squeezed at the very start by the next one goes.
    if labels.len() > 1 && labels[1].0 - labels[0].0 < 3 {
        labels.remove(0);
    }
    labels
}

/// 1748 → "1,748".
pub fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "Thu, Nov 20, 2026", for a cell's tooltip.
fn day_label(language: Language, day: i64) -> String {
    let (year, month, date) = civil_from_days(day);
    tr(language, "{weekday}, {month} {day}, {year}")
        .replace("{weekday}", tr(language, WEEKDAYS[weekday(day)]))
        .replace("{month}", tr(language, MONTHS[month as usize - 1]))
        .replace("{day}", &date.to_string())
        .replace("{year}", &year.to_string())
}

/// "3 commits on Thu, Nov 20, 2026".
pub fn day_summary(language: Language, day: &Day, unit: Unit) -> String {
    let key = if day.count == 0 {
        "No {unit} on {date}"
    } else {
        "{count} {unit} on {date}"
    };
    tr(language, key)
        .replace("{count}", &thousands(day.count))
        .replace("{unit}", unit.word(language, day.count))
        .replace("{date}", &day_label(language, day.day))
}

/// "1,748 contributions in the last year", or "in 20 weeks" when a narrow
/// pane shows fewer.
pub fn heading(language: Language, count: u32, weeks: usize, unit: Unit) -> String {
    let key = if weeks >= WEEKS {
        "{count} {unit} in the last year"
    } else {
        "{count} {unit} in {weeks} weeks"
    };
    tr(language, key)
        .replace("{count}", &thousands(count))
        .replace("{unit}", unit.word(language, count))
        .replace("{weeks}", &weeks.to_string())
}

// ── GitHub ──────────────────────────────────────────────────────────────────

/// The account behind a GitHub noreply address: `123+login@…` or the
/// older `login@users.noreply.github.com`.
pub fn noreply_login(email: &str) -> Option<String> {
    let local = email
        .trim()
        .to_lowercase()
        .strip_suffix("@users.noreply.github.com")?
        .to_string();
    let login = local.split_once('+').map_or(local.as_str(), |(_, l)| l);
    valid_login(login).then(|| login.to_string())
}

/// A GitHub login: letters, digits and single inner hyphens, up to 39.
pub fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !login.starts_with('-')
        && !login.ends_with('-')
}

/// The page GitHub draws a profile's contribution graph from.
pub fn contributions_url(login: &str) -> String {
    format!("https://github.com/users/{login}/contributions")
}

/// Contributions per day from [`contributions_url`]'s HTML: each day is a
/// `<td data-date="…" id="…">`, its count in the `<tool-tip for="…">`
/// ("No contributions on …", "12 contributions on …").
pub fn parse_contributions(html: &str) -> Vec<(i64, u32)> {
    let attr = |tag: &str, name: &str| -> Option<String> {
        let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
        let end = tag[start..].find('"')? + start;
        Some(tag[start..end].to_string())
    };
    let mut dates: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for chunk in html.split("<td").skip(1) {
        let tag = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
        if let (Some(date), Some(id)) = (attr(tag, "data-date"), attr(tag, "id")) {
            let part = |r: std::ops::Range<usize>| date.get(r)?.parse::<i64>().ok();
            if let (Some(y), Some(m), Some(d)) = (part(0..4), part(5..7), part(8..10)) {
                dates.insert(id, days_from_civil(y, m, d));
            }
        }
    }
    let mut days = Vec::new();
    for chunk in html.split("<tool-tip").skip(1) {
        let Some(close) = chunk.find('>') else {
            continue;
        };
        let (tag, rest) = chunk.split_at(close);
        let text = rest[1..].split("</tool-tip>").next().unwrap_or_default();
        let Some(day) = attr(tag, "for").and_then(|id| dates.get(&id).copied()) else {
            continue;
        };
        let count = if text.trim_start().starts_with("No ") {
            0
        } else {
            text.split_whitespace()
                .next()
                .and_then(|n| n.replace(',', "").parse().ok())
                .unwrap_or(0)
        };
        days.push((day, count));
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i64, m: i64, d: i64, hour: i64) -> i64 {
        days_from_civil(y, m, d) * DAY + hour * 3600
    }

    #[test]
    fn the_grid_ends_with_todays_week_and_counts_local_days() {
        // Friday 2026-11-20, 10:00 UTC; the user is at UTC−6.
        let offset = -6 * 3600;
        let today = local_day(at(2026, 11, 20, 10), offset);
        let times = [
            // 02:00 UTC on the 20th is still the 19th at UTC−6.
            at(2026, 11, 20, 2),
            at(2026, 11, 20, 9),
            at(2026, 11, 20, 9),
            // Older than the grid.
            at(2025, 1, 1, 12),
        ];
        let grid = activity(&count_days(&times, offset), today);
        assert_eq!(grid.weeks.len(), WEEKS);
        let last = grid.weeks.last().unwrap();
        assert_eq!(weekday(last[0].day), 0, "weeks start on Sunday");
        let cell = |day: i64| grid.weeks.iter().flatten().find(|d| d.day == day).unwrap();
        assert_eq!(cell(today).count, 2);
        assert_eq!(cell(today - 1).count, 1);
        assert!(cell(today + 1).future && !cell(today).future);
        assert_eq!(total(&grid.weeks), 3);
    }

    #[test]
    fn shades_are_quartiles_of_the_busy_days() {
        let today = days_from_civil(2026, 11, 20);
        // Busy days with 1, 2, 3, 4 and one outlier of 40.
        let counts: Vec<(i64, u32)> = [1, 2, 3, 4, 40]
            .iter()
            .enumerate()
            .map(|(i, c)| (today - i as i64, *c))
            .collect();
        let grid = activity(&counts, today);
        let level = |count: u32| {
            grid.weeks
                .iter()
                .flatten()
                .find(|d| d.count == count)
                .unwrap()
                .level
        };
        assert_eq!(level(0), 0);
        // The outlier doesn't push every other day down to the first shade.
        assert_eq!(
            [level(1), level(2), level(3), level(4), level(40)],
            [1, 1, 2, 3, 4]
        );
    }

    #[test]
    fn months_are_named_where_they_begin() {
        let grid = activity(&[], days_from_civil(2026, 11, 20));
        let labels = month_labels(Language::English, &grid.weeks);
        let names: Vec<&str> = labels.iter().map(|(_, m)| *m).collect();
        // A year back from late November, as GitHub draws it: Nov … Nov.
        assert_eq!(names.first(), Some(&"Nov"), "{names:?}");
        assert_eq!(names.len(), 13, "{names:?}");
        assert!(labels.windows(2).all(|w| w[1].0 - w[0].0 >= 3));
    }

    #[test]
    fn words_read() {
        assert_eq!(thousands(1748), "1,748");
        assert_eq!(thousands(12), "12");
        assert_eq!(thousands(1_000_000), "1,000,000");
        assert_eq!(
            heading(Language::English, 1748, WEEKS, Unit::Contributions),
            "1,748 contributions in the last year"
        );
        assert_eq!(
            heading(Language::English, 1, 20, Unit::Commits),
            "1 commit in 20 weeks"
        );
        let day = Day {
            day: days_from_civil(2026, 11, 20),
            count: 0,
            level: 0,
            future: false,
        };
        assert_eq!(
            day_summary(Language::English, &day, Unit::Commits),
            "No commits on Fri, Nov 20, 2026"
        );
        assert_eq!(
            parse_times("1763600000\nbad\n1763600001\n"),
            [1763600000, 1763600001]
        );
    }

    #[test]
    fn github_logins_and_its_calendar_page() {
        assert_eq!(
            noreply_login("123+Maya@users.noreply.github.com").as_deref(),
            Some("maya")
        );
        assert_eq!(
            noreply_login("maya@users.noreply.github.com").as_deref(),
            Some("maya")
        );
        assert_eq!(noreply_login("maya@example.com"), None);
        assert!(!valid_login("a/b") && !valid_login("-a") && valid_login("ismael-7824"));

        let html = r#"<table><tr>
<td tabindex="0" data-ix="0" style="width: 10px" data-date="2026-11-19" id="contribution-day-component-4-51" data-level="0" class="ContributionCalendar-day"></td>
<td tabindex="0" data-ix="0" style="width: 10px" data-date="2026-11-20" id="contribution-day-component-5-51" data-level="4" class="ContributionCalendar-day"></td>
</tr></table>
<tool-tip id="t1" for="contribution-day-component-4-51" popover="manual">No contributions on November 19th.</tool-tip>
<tool-tip id="t2" for="contribution-day-component-5-51" popover="manual">1,204 contributions on November 20th.</tool-tip>
<tool-tip id="t3" for="something-else">Not a day</tool-tip>"#;
        let mut days = parse_contributions(html);
        days.sort_unstable();
        let nov = |d| days_from_civil(2026, 11, d);
        assert_eq!(days, [(nov(19), 0), (nov(20), 1204)]);
    }
}
