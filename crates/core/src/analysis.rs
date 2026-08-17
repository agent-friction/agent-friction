use crate::PermissionStat;
use crate::collapse::{CollapsedStat, collapse};
use serde::Serialize;

const ALLOW_ALWAYS_TO_ALLOW_ONCE_APPROX_RATIO: i64 = 5;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Verdict {
    Allow { confidence: f64 },
    KeepAsking { reason: String },
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Suggestion {
    pub tool: String,
    pub pattern: String,
    pub verdict: Verdict,
    /// The observed patterns this covers. More than one means the pattern was
    /// widened, and these are the evidence for it -- worth carrying, since a
    /// rule the user cannot audit is a rule they cannot trust.
    pub members: Vec<String>,
    /// Prompts this rule would have answered.
    pub events: i64,
}

/// Folds the raw stats into the coarsest rules the evidence supports, then
/// judges those. Judging raw stats directly asks every over-specific pattern to
/// clear the bar on its own, which almost none of them ever do.
pub fn analyze(stats: &[PermissionStat]) -> Vec<Suggestion> {
    let mut suggestions: Vec<Suggestion> =
        collapse(stats).iter().map(suggestion_from_stat).collect();

    // Recommendations first, then what deserves a second look, each ordered by
    // how much friction it accounts for. The head of this list should be the
    // lines worth pasting into a config.
    suggestions.sort_by(|a, b| {
        a.verdict
            .rank()
            .cmp(&b.verdict.rank())
            .then_with(|| b.events.cmp(&a.events))
            .then_with(|| a.tool.cmp(&b.tool))
            .then_with(|| a.pattern.cmp(&b.pattern))
    });
    suggestions
}

impl Verdict {
    fn rank(&self) -> u8 {
        match self {
            Verdict::Allow { .. } => 0,
            Verdict::KeepAsking { .. } => 1,
            Verdict::Insufficient => 2,
        }
    }
}

fn suggestion_from_stat(stat: &CollapsedStat) -> Suggestion {
    let mut suggestion = Suggestion {
        tool: stat.tool.clone(),
        pattern: stat.pattern.clone(),
        verdict: Verdict::Insufficient,
        members: stat.members.clone(),
        events: stat.total_events(),
    };

    let approx_approvals =
        stat.allow_once + (stat.allow_always * ALLOW_ALWAYS_TO_ALLOW_ONCE_APPROX_RATIO);
    if stat.deny > 0 {
        suggestion.verdict = Verdict::KeepAsking {
            reason: "Tool was denied at least once".into(),
        }
    } else if approx_approvals >= 30 {
        // 30 is statistically likely from central limit theorem to
        // be a good average here
        suggestion.verdict = Verdict::Allow {
            // confidence is how far above 30 we are, with 100 as a ceiling where we would expect
            // 100% confidence
            //
            // Starting at 30 we have a 50% confidence in the answer
            // TODO: tune this
            confidence: 0.5 + ((approx_approvals as f64 - 30.0) / 140.0).clamp(0.0, 0.5),
        }
    }

    suggestion
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::PermissionStat;

    fn stat(pattern: &str, allow_once: i64, allow_always: i64, deny: i64) -> PermissionStat {
        PermissionStat {
            tool: "bash".into(),
            pattern: pattern.into(),
            allow_once,
            allow_always,
            deny,
        }
    }

    #[test]
    fn analysis_approval_thresholds() {
        // None of these families are broad enough to collapse, so each is
        // judged on its own and the ordering is the ranking.
        let stats = vec![
            stat("git status *", 0, 20, 0),
            stat("git push *", 20, 2, 0),
            stat("go run *", 17, 1, 0),
            stat("rm *", 20, 2, 1),
            stat("rm -rf *", 0, 0, 3),
        ];

        assert_eq!(
            analyze(&stats),
            vec![
                Suggestion {
                    tool: "bash".into(),
                    pattern: "git push *".into(),
                    verdict: Verdict::Allow { confidence: 0.5 },
                    members: vec!["git push *".into()],
                    events: 22,
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "git status *".into(),
                    verdict: Verdict::Allow { confidence: 1.0 },
                    members: vec!["git status *".into()],
                    events: 20,
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "rm *".into(),
                    verdict: Verdict::KeepAsking {
                        reason: "Tool was denied at least once".into()
                    },
                    members: vec!["rm *".into()],
                    events: 23,
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "rm -rf *".into(),
                    verdict: Verdict::KeepAsking {
                        reason: "Tool was denied at least once".into()
                    },
                    members: vec!["rm -rf *".into()],
                    events: 3,
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "go run *".into(),
                    verdict: Verdict::Insufficient,
                    members: vec!["go run *".into()],
                    events: 18,
                },
            ]
        );
    }

    /// Why collapsing exists: three flag variants that would each be dismissed
    /// as Insufficient (12, 11 and 10 approvals against a bar of 30) clear it
    /// comfortably once folded into the rule they all support.
    #[test]
    fn collapsing_lets_scattered_evidence_reach_a_verdict() {
        let stats = vec![
            stat("git status --short", 12, 0, 0),
            stat("git status --porcelain", 11, 0, 0),
            stat("git status", 10, 0, 0),
        ];

        let suggestions = analyze(&stats);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].pattern, "git status *");
        assert_eq!(suggestions[0].events, 33);
        assert_eq!(suggestions[0].members.len(), 3);
        assert!(matches!(suggestions[0].verdict, Verdict::Allow { .. }));
    }
}
