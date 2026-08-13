use crate::PermissionStat;
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
}

pub fn analyze(stats: &[PermissionStat]) -> Vec<Suggestion> {
    stats.iter().map(suggestion_from_stat).collect()
}

fn suggestion_from_stat(stat: &PermissionStat) -> Suggestion {
    let mut suggestion = Suggestion {
        tool: stat.tool.clone(),
        pattern: stat.pattern.clone(),
        verdict: Verdict::Insufficient,
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

    #[test]
    fn analysis_approval_thresholds() {
        let stats = vec![
            PermissionStat {
                tool: "bash".into(),
                pattern: "git status *".into(),
                allow_once: 0,
                allow_always: 20,
                deny: 0,
            },
            PermissionStat {
                tool: "bash".into(),
                pattern: "git push *".into(),
                allow_once: 20,
                allow_always: 2,
                deny: 0,
            },
            PermissionStat {
                tool: "bash".into(),
                pattern: "go run *".into(),
                allow_once: 17,
                allow_always: 1,
                deny: 0,
            },
            PermissionStat {
                tool: "bash".into(),
                pattern: "rm *".into(),
                allow_once: 20,
                allow_always: 2,
                deny: 1,
            },
            PermissionStat {
                tool: "bash".into(),
                pattern: "rm -rf *".into(),
                allow_once: 0,
                allow_always: 0,
                deny: 3,
            },
        ];

        assert_eq!(
            analyze(&stats),
            vec![
                Suggestion {
                    tool: "bash".into(),
                    pattern: "git status *".into(),
                    verdict: Verdict::Allow { confidence: 1.0 }
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "git push *".into(),
                    verdict: Verdict::Allow { confidence: 0.5 }
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "go run *".into(),
                    verdict: Verdict::Insufficient,
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "rm *".into(),
                    verdict: Verdict::KeepAsking {
                        reason: "Tool was denied at least once".into()
                    }
                },
                Suggestion {
                    tool: "bash".into(),
                    pattern: "rm -rf *".into(),
                    verdict: Verdict::KeepAsking {
                        reason: "Tool was denied at least once".into()
                    }
                },
            ]
        );
    }
}
