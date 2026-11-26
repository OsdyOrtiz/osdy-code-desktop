//! Work an agent keeps running besides its own turn (subagents and
//! background jobs), and the light the window shows for an agent, a tab or
//! a workspace.

use std::ops::AddAssign;

/// What runs in the background of an agent, a tab or a workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Background {
    pub subagents: usize,
    pub jobs: usize,
}

impl Background {
    /// Only what runs now: subagents whose status is `running` and jobs
    /// still running. Queued or waiting subagents don't count.
    pub fn count<'a>(
        subagent_statuses: impl IntoIterator<Item = &'a str>,
        jobs_running: impl IntoIterator<Item = bool>,
    ) -> Self {
        Self {
            subagents: subagent_statuses
                .into_iter()
                .filter(|status| *status == "running")
                .count(),
            jobs: jobs_running.into_iter().filter(|running| *running).count(),
        }
    }

    pub fn is_idle(self) -> bool {
        self.subagents == 0 && self.jobs == 0
    }

    /// "1 subagent and 2 jobs in background"; `None` when nothing runs.
    pub fn describe(self) -> Option<String> {
        let parts: Vec<String> = [
            (self.subagents, "subagent", "subagents"),
            (self.jobs, "job", "jobs"),
        ]
        .into_iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, one, many)| plural(count, one, many))
        .collect();
        (!parts.is_empty()).then(|| format!("{} in background", parts.join(" and ")))
    }
}

impl AddAssign for Background {
    fn add_assign(&mut self, other: Self) {
        self.subagents += other.subagents;
        self.jobs += other.jobs;
    }
}

/// The one light an agent, a tab or a workspace shows: the most urgent
/// activity wins (a dialog waiting on you, then the agent's own turn, then
/// subagents, then background jobs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    NeedsYou,
    Working,
    Subagents,
    Jobs,
}

impl Light {
    pub fn of(needs_you: bool, working: bool, background: Background) -> Option<Self> {
        if needs_you {
            Some(Self::NeedsYou)
        } else if working {
            Some(Self::Working)
        } else if background.subagents > 0 {
            Some(Self::Subagents)
        } else if background.jobs > 0 {
            Some(Self::Jobs)
        } else {
            None
        }
    }

    /// Whether the light breathes: everything that runs, not a wait on you.
    pub fn pulses(self) -> bool {
        self != Self::NeedsYou
    }

    /// What the light says about a single agent or tab.
    pub fn what(self) -> &'static str {
        match self {
            Self::NeedsYou => "An agent here needs you",
            Self::Working => "Agents are working here",
            Self::Subagents => "Subagents are running in background",
            Self::Jobs => "Jobs are running in background",
        }
    }

    /// The switcher's badge for `count` agents, subagents or jobs.
    pub fn badge(self, count: usize) -> String {
        match (self, count) {
            (Self::NeedsYou, 1) => "needs you".to_string(),
            (Self::NeedsYou, n) => format!("{n} need you"),
            (Self::Working, 1) => "working".to_string(),
            (Self::Working, n) => format!("{n} working"),
            (Self::Subagents, n) => plural(n, "subagent", "subagents"),
            (Self::Jobs, n) => plural(n, "job", "jobs"),
        }
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_running_subagents_and_jobs() {
        let background = Background::count(
            ["running", "queued", "waiting", "completed", "running"],
            [true, false, true, true],
        );
        assert_eq!(
            background,
            Background {
                subagents: 2,
                jobs: 3
            }
        );
        assert!(!background.is_idle());
        assert!(Background::count([], []).is_idle());
    }

    #[test]
    fn adds_up_across_agents() {
        let mut total = Background::default();
        total += Background {
            subagents: 1,
            jobs: 0,
        };
        total += Background {
            subagents: 2,
            jobs: 3,
        };
        assert_eq!(
            total,
            Background {
                subagents: 3,
                jobs: 3
            }
        );
    }

    #[test]
    fn describes_what_runs_in_background() {
        let describe = |subagents, jobs| Background { subagents, jobs }.describe();
        assert_eq!(describe(0, 0), None);
        assert_eq!(describe(1, 0).as_deref(), Some("1 subagent in background"));
        assert_eq!(describe(0, 2).as_deref(), Some("2 jobs in background"));
        assert_eq!(
            describe(2, 1).as_deref(),
            Some("2 subagents and 1 job in background")
        );
    }

    #[test]
    fn the_most_urgent_activity_lights_up() {
        let both = Background {
            subagents: 1,
            jobs: 1,
        };
        let jobs = Background {
            subagents: 0,
            jobs: 1,
        };
        assert_eq!(Light::of(true, true, both), Some(Light::NeedsYou));
        assert_eq!(Light::of(false, true, both), Some(Light::Working));
        assert_eq!(Light::of(false, false, both), Some(Light::Subagents));
        assert_eq!(Light::of(false, false, jobs), Some(Light::Jobs));
        assert_eq!(Light::of(false, false, Background::default()), None);
    }

    #[test]
    fn only_a_wait_on_you_stays_still() {
        assert!(!Light::NeedsYou.pulses());
        assert!(Light::Working.pulses());
        assert!(Light::Subagents.pulses());
        assert!(Light::Jobs.pulses());
    }

    #[test]
    fn badges_count_what_lights_up() {
        assert_eq!(Light::NeedsYou.badge(1), "needs you");
        assert_eq!(Light::NeedsYou.badge(2), "2 need you");
        assert_eq!(Light::Working.badge(1), "working");
        assert_eq!(Light::Working.badge(3), "3 working");
        assert_eq!(Light::Subagents.badge(1), "1 subagent");
        assert_eq!(Light::Jobs.badge(2), "2 jobs");
    }
}
