use anyhow::{Ok, Result};

use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Table};

use crate::cli::AnalyzeArgs;
use crate::display::abbreviate;
use agent_friction_core::{Db, Limits, Scope, Suggestion, Verdict, analyze};

/// Deliberately raw. The point right now is to see what the collapsing and the
/// thresholds actually produce against real data, so this prints every verdict
/// including the ones it has nothing to say about, rather than presenting a
/// short list of recommendations as if they were trustworthy yet.
pub fn run(db: &Db, args: AnalyzeArgs) -> Result<()> {
    // Unfiltered on purpose. `--min-count` and `--limit` trim the *output*, and
    // pushing them into the query would instead starve the collapsing: the
    // sparse one-off patterns they would discard are exactly the ones that sum
    // into a rule worth suggesting.
    let stats = db.get_permission_stats(
        args.common.since(),
        Scope::from(args.common.repo.clone()),
        Limits::default(),
    )?;

    let mut suggestions = analyze(&stats);
    suggestions.retain(|s| s.events >= args.common.min_count);
    if let Some(limit) = args.common.limit {
        suggestions.truncate(limit.max(0) as usize);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
        return Ok(());
    }

    println!("\x1b[1mPermission Suggestions\x1b[0m");
    println!("{}", build_table(&suggestions));
    Ok(())
}

fn build_table(suggestions: &[Suggestion]) -> Table {
    let mut table = Table::new();

    table.load_style(UTF8_FULL).set_header(vec![
        Cell::new("Tool").fg(Color::Blue),
        Cell::new("Pattern").fg(Color::Blue),
        Cell::new("Verdict").fg(Color::Blue),
        Cell::new("Events").fg(Color::Blue),
        Cell::new("Evidence").fg(Color::Blue),
    ]);

    for s in suggestions {
        table.add_row(vec![
            Cell::new(&s.tool),
            Cell::new(abbreviate(&s.pattern)),
            verdict_cell(&s.verdict),
            Cell::new(s.events),
            Cell::new(evidence(s)),
        ]);
    }

    table
}

fn verdict_cell(verdict: &Verdict) -> Cell {
    match verdict {
        Verdict::Allow { confidence } => {
            Cell::new(format!("allow ({:.0}%)", confidence * 100.0)).fg(Color::Green)
        }
        Verdict::KeepAsking { reason } => Cell::new(format!("keep asking: {reason}")).fg(Color::Red),
        Verdict::Insufficient => Cell::new("insufficient data").fg(Color::DarkGrey),
    }
}

/// What a widened pattern was inferred from. A rule the user cannot audit is a
/// rule they cannot trust, so the observed patterns travel with it.
fn evidence(suggestion: &Suggestion) -> String {
    match suggestion.members.len() {
        0 | 1 => String::from("-"),
        _ => suggestion
            .members
            .iter()
            .map(|m| abbreviate(m))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
