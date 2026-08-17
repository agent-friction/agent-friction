use anyhow::{Ok, Result};

use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Table};

use crate::cli::{CommonStatsArgs, StatsArgs, StatsSubcommands};
use crate::display::abbreviate;
use agent_friction_core::{Db, FailureStat, PermissionStat, Scope};

pub fn run(db: &Db, stats: StatsArgs) -> Result<()> {
    match stats.sub_action {
        Some(StatsSubcommands::Permissions) => run_permissions(db, stats.common),
        Some(StatsSubcommands::Failures) => run_failures(db, stats.common),
        None => run_all(db, stats.common),
    }
}

fn run_permissions(db: &Db, common: CommonStatsArgs) -> Result<()> {
    let (since, limits) = (common.since(), common.limits());
    let stats = db.get_permission_stats(since, Scope::from(common.repo), limits)?;
    print_permissions_table(stats);
    Ok(())
}

fn run_failures(db: &Db, common: CommonStatsArgs) -> Result<()> {
    let (since, limits) = (common.since(), common.limits());
    let stats = db.get_failure_stats(since, Scope::from(common.repo), limits)?;
    print_failures_table(stats);
    Ok(())
}

fn run_all(db: &Db, common: CommonStatsArgs) -> Result<()> {
    let (since, limits) = (common.since(), common.limits());
    let permission_stats =
        db.get_permission_stats(since, Scope::from(common.repo.clone()), limits)?;
    print_permissions_table(permission_stats);
    let failure_stats = db.get_failure_stats(since, Scope::from(common.repo), limits)?;
    print_failures_table(failure_stats);
    Ok(())
}

fn print_permissions_table(stats: Vec<PermissionStat>) {
    let table = build_permissions_table(stats);
    println!("\x1b[1mPermissions Stats\x1b[0m");
    println!("{}", table);
}

fn build_permissions_table(stats: Vec<PermissionStat>) -> Table {
    let mut table = Table::new();

    table.load_style(UTF8_FULL).set_header(vec![
        Cell::new("Tool").fg(Color::Blue),
        Cell::new("Always Allow").fg(Color::Green),
        Cell::new("Allow Once").fg(Color::Yellow),
        Cell::new("Deny").fg(Color::Red),
        Cell::new("Pattern").fg(Color::Blue),
    ]);

    for stat in stats {
        table.add_row(vec![
            stat.tool,
            stat.allow_always.to_string(),
            stat.allow_once.to_string(),
            stat.deny.to_string(),
            abbreviate(&stat.pattern),
        ]);
    }

    table
}

fn print_failures_table(stats: Vec<FailureStat>) {
    let table = build_failures_table(stats);
    println!("\x1b[1mTool Failures Stats\x1b[0m");
    println!("{}", table);
}

fn build_failures_table(stats: Vec<FailureStat>) -> Table {
    let mut table = Table::new();

    table.load_style(UTF8_FULL).set_header(vec![
        Cell::new("Tool").fg(Color::Blue),
        Cell::new("Count").fg(Color::Blue),
        Cell::new("Error Pattern").fg(Color::Red),
        Cell::new("Example").fg(Color::Red),
        Cell::new("Source").fg(Color::Blue),
    ]);

    for stat in stats {
        table.add_row(vec![
            stat.tool,
            stat.count.to_string(),
            abbreviate(&stat.error_pattern),
            abbreviate(&stat.example),
            stat.source.to_string(),
        ]);
    }

    table
}
