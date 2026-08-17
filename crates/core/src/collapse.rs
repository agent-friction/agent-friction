//! Folding observed permission patterns into the coarsest rule the evidence
//! supports.
//!
//! Recorded patterns are far too specific to ever accumulate evidence: `git
//! status --short` and `git status --porcelain` are counted apart, so neither
//! ever reaches the bar the analysis needs to recommend anything. Collapsing is
//! what makes that evidence add up.
//!
//! Collapsing widens the authority a rule grants, and that asymmetry drives
//! every rule here. A candidate is always a *prefix* of the command's tokens
//! with `*` appended, because that is the shape the agent's permission config
//! can actually express -- so the search is a prefix trie over argv, not
//! general pattern mining.
//!
//! Only bash patterns are collapsed for now. Path-shaped patterns (`read`,
//! `edit`) would fall out of the same trie tokenized on `/`, but the glob
//! dialect for those is worth confirming against real data first.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::PermissionStat;

/// Never collapse to a bare command. `git *` includes `git push --force`, and
/// the subcommand is where the semantics live.
const MIN_TOKENS: usize = 2;

/// How many *distinct* observed patterns a node needs before widening it.
/// Volume justifies confidence; breadth justifies generalization. Seeing `git
/// status --short` forty times is evidence about `git status --short`, not
/// about `git status *`.
const MIN_DISTINCT: usize = 3;

/// A trailing `*` after a redirect or a substitution grants far more than it
/// looks like it does, so anything shell-ish is left exactly as observed.
const SHELL_METACHARS: &[char] = &['>', '<', '&', '|', ';', '$', '`', '(', ')'];

const BASH: &str = "bash";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollapsedStat {
    pub tool: String,
    /// The rule, possibly widened: `git status *`.
    pub pattern: String,
    pub allow_once: i64,
    pub allow_always: i64,
    pub deny: i64,
    /// The observed patterns folded into `pattern`, which is the evidence the
    /// widening rests on. A single member means nothing was collapsed.
    pub members: Vec<String>,
}

impl CollapsedStat {
    pub fn is_collapsed(&self) -> bool {
        self.members.len() > 1
    }

    /// Prompts this rule would have answered -- the friction it saves.
    pub fn total_events(&self) -> i64 {
        self.allow_once + self.allow_always + self.deny
    }
}

struct Entry<'a> {
    tokens: Vec<&'a str>,
    stat: &'a PermissionStat,
}

pub fn collapse(stats: &[PermissionStat]) -> Vec<CollapsedStat> {
    let mut by_tool: BTreeMap<&str, Vec<&PermissionStat>> = BTreeMap::new();
    for stat in stats {
        by_tool.entry(stat.tool.as_str()).or_default().push(stat);
    }

    let mut out = Vec::new();
    for (tool, tool_stats) in by_tool {
        if tool != BASH {
            out.extend(tool_stats.into_iter().map(leaf));
            continue;
        }

        let mut entries = Vec::new();
        for stat in tool_stats {
            match tokenize(&stat.pattern) {
                // Nothing to build a prefix out of, so it stands on its own.
                Some(tokens) if !tokens.is_empty() => entries.push(Entry { tokens, stat }),
                _ => out.push(leaf(stat)),
            }
        }

        entries.sort_by(|a, b| a.tokens.cmp(&b.tokens));
        for group in group_by_token(&entries, 0) {
            collapse_group(group, 1, &mut out);
        }
    }

    out
}

/// Splits a pattern into argv tokens, dropping the trailing `*` the agent
/// already added. Returns `None` for anything carrying shell syntax.
fn tokenize(pattern: &str) -> Option<Vec<&str>> {
    if pattern.contains(SHELL_METACHARS) {
        return None;
    }

    let mut tokens: Vec<&str> = pattern.split_whitespace().collect();
    while tokens.last() == Some(&"*") {
        tokens.pop();
    }
    Some(tokens)
}

/// `entries` all share their first `depth` tokens. Either the shared prefix is
/// widened into one rule, or we descend a token and try again.
fn collapse_group(entries: &[Entry<'_>], depth: usize, out: &mut Vec<CollapsedStat>) {
    if entries.len() == 1 {
        out.push(leaf(entries[0].stat));
        return;
    }

    let deny: i64 = entries.iter().map(|e| e.stat.deny).sum();

    // A deny poisons every ancestor, not just the node it landed on: one
    // `git push --force` refusal has to block `git push *` as well.
    if widenable(entries, depth) && entries.len() >= MIN_DISTINCT && deny == 0 {
        out.push(collapsed(entries, depth));
        return;
    }

    // The prefix as typed (`git status` with no arguments) is its own
    // observation, not a parent of the longer ones.
    let (exact, longer): (Vec<_>, Vec<_>) = entries.iter().partition(|e| e.tokens.len() == depth);
    out.extend(exact.into_iter().map(|e| leaf(e.stat)));

    let longer: Vec<Entry<'_>> = longer
        .into_iter()
        .map(|e| Entry {
            tokens: e.tokens.clone(),
            stat: e.stat,
        })
        .collect();

    for group in group_by_token(&longer, depth) {
        collapse_group(group, depth + 1, out);
    }
}

/// Splits a token-sorted slice into runs sharing `tokens[index]`.
fn group_by_token<'e, 'a>(entries: &'e [Entry<'a>], index: usize) -> Vec<&'e [Entry<'a>]> {
    let mut groups = Vec::new();
    let mut start = 0;
    for i in 1..=entries.len() {
        if i == entries.len() || entries[i].tokens[index] != entries[start].tokens[index] {
            groups.push(&entries[start..i]);
            start = i;
        }
    }
    groups
}

/// Whether the shared prefix may be widened into a rule.
///
/// Past the first token this is just depth. At the first token it is not: `git
/// *` would cover `git push --force`, so a bare command normally stays shut.
///
/// The thing that makes `git` dangerous, though, is that its next token is a
/// *subcommand* that changes what the program does -- and `tail`, `head` and
/// `ls` have no such thing, their next token is only ever a flag. Depth alone
/// cannot tell those apart, but the observed variation can: if every command in
/// the family differs from the prefix by flags and nothing else, widening adds
/// no new verb and no new operand. `tail -15/-20/-60` qualifies; `git
/// status/log/diff` does not; neither does `rm -f notes.txt`, whose operand is
/// exactly the part that must not be wildcarded.
fn widenable(entries: &[Entry<'_>], depth: usize) -> bool {
    if depth >= MIN_TOKENS {
        return true;
    }

    entries
        .iter()
        .all(|e| e.tokens[depth..].iter().all(|t| is_flag(t)))
}

fn is_flag(token: &str) -> bool {
    token.starts_with('-') && token.len() > 1
}

fn leaf(stat: &PermissionStat) -> CollapsedStat {
    CollapsedStat {
        tool: stat.tool.clone(),
        pattern: stat.pattern.clone(),
        allow_once: stat.allow_once,
        allow_always: stat.allow_always,
        deny: stat.deny,
        members: vec![stat.pattern.clone()],
    }
}

fn collapsed(entries: &[Entry<'_>], depth: usize) -> CollapsedStat {
    let prefix = entries[0].tokens[..depth].join(" ");
    CollapsedStat {
        tool: entries[0].stat.tool.clone(),
        pattern: format!("{prefix} *"),
        allow_once: entries.iter().map(|e| e.stat.allow_once).sum(),
        allow_always: entries.iter().map(|e| e.stat.allow_always).sum(),
        deny: entries.iter().map(|e| e.stat.deny).sum(),
        members: entries.iter().map(|e| e.stat.pattern.clone()).collect(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn stat(tool: &str, pattern: &str, allow_once: i64, deny: i64) -> PermissionStat {
        PermissionStat {
            tool: tool.into(),
            pattern: pattern.into(),
            allow_once,
            allow_always: 0,
            deny,
        }
    }

    fn find<'a>(out: &'a [CollapsedStat], pattern: &str) -> Option<&'a CollapsedStat> {
        out.iter().find(|c| c.pattern == pattern)
    }

    #[test]
    fn folds_a_broad_subcommand_and_sums_its_evidence() {
        let out = collapse(&[
            stat("bash", "git status --short", 1, 0),
            stat("bash", "git status --porcelain", 2, 0),
            stat("bash", "git status", 3, 0),
        ]);

        assert_eq!(out.len(), 1);
        let c = find(&out, "git status *").unwrap();
        assert_eq!(c.allow_once, 6);
        assert_eq!(c.members.len(), 3);
        assert!(c.is_collapsed());
    }

    #[test]
    fn keeps_thin_evidence_specific() {
        // Two distinct patterns is not enough to claim anything about the rest
        // of `git status`.
        let out = collapse(&[
            stat("bash", "git status --short", 20, 0),
            stat("bash", "git status", 20, 0),
        ]);

        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|c| !c.is_collapsed()));
    }

    #[test]
    fn never_widens_to_a_bare_command() {
        let out = collapse(&[
            stat("bash", "git status", 5, 0),
            stat("bash", "git log", 5, 0),
            stat("bash", "git diff", 5, 0),
        ]);

        assert!(find(&out, "git *").is_none());
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn widens_a_bare_command_that_only_varies_by_flags() {
        // `tail` has no subcommands, so `tail *` grants nothing that
        // `tail -15` did not already.
        let out = collapse(&[
            stat("bash", "tail -15", 1, 0),
            stat("bash", "tail -20", 1, 0),
            stat("bash", "tail -60", 1, 0),
        ]);

        let c = find(&out, "tail *").unwrap();
        assert_eq!(c.members.len(), 3);
    }

    #[test]
    fn an_operand_stops_a_bare_command_from_widening() {
        // The operand is the dangerous part, and it is exactly what the
        // wildcard would swallow.
        let out = collapse(&[
            stat("bash", "rm -f notes.txt", 5, 0),
            stat("bash", "rm -f scratch.txt", 5, 0),
            stat("bash", "rm -rf build", 5, 0),
        ]);

        assert!(find(&out, "rm *").is_none());
        assert!(find(&out, "rm -f *").is_none());
    }

    #[test]
    fn a_deny_anywhere_blocks_the_whole_subtree() {
        let out = collapse(&[
            stat("bash", "git push --force", 0, 1),
            stat("bash", "git push origin", 5, 0),
            stat("bash", "git push --tags", 5, 0),
            stat("bash", "git push -u", 5, 0),
        ]);

        assert!(find(&out, "git push *").is_none());
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn shell_syntax_is_left_alone() {
        let out = collapse(&[
            stat("bash", "make test 2>&1", 1, 0),
            stat("bash", "make test --verbose", 1, 0),
            stat("bash", "make test -j4", 1, 0),
            stat("bash", "make test -k", 1, 0),
        ]);

        // The three clean ones fold; the redirect stays verbatim.
        let c = find(&out, "make test *").unwrap();
        assert_eq!(c.members.len(), 3);
        assert!(find(&out, "make test 2>&1").is_some());
    }

    #[test]
    fn only_bash_is_collapsed_for_now() {
        let out = collapse(&[
            stat("read", "crates/core/src/query.rs", 5, 0),
            stat("read", "crates/core/src/store.rs", 5, 0),
            stat("read", "crates/core/src/db.rs", 5, 0),
        ]);

        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|c| !c.is_collapsed()));
    }

    #[test]
    fn independent_families_do_not_bleed_together() {
        let out = collapse(&[
            stat("bash", "cargo test --workspace", 1, 0),
            stat("bash", "cargo test -p core", 1, 0),
            stat("bash", "cargo test --lib", 1, 0),
            stat("bash", "cargo build --release", 1, 0),
        ]);

        assert!(find(&out, "cargo test *").is_some());
        assert!(find(&out, "cargo build --release").is_some());
        assert!(find(&out, "cargo *").is_none());
    }
}
