use std::fmt::Write;

use app::inspect::{Difference, Row};
use app::sync::SyncOutcome;
use app::transaction::{Applied, Record};
use domain::plan::{Landing, Realized, Transfer};

#[must_use]
pub fn rows(rows: &[Row]) -> String {
    let cells: Vec<[String; 5]> = rows
        .iter()
        .map(|row| {
            [
                row.container.to_string(),
                row.placement.to_string(),
                row.code.map(|code| code.to_string()).unwrap_or_default(),
                row.quality.to_string(),
                row.fingerprint.to_string(),
            ]
        })
        .collect();
    table(&cells)
}

#[must_use]
pub fn differences(differences: &[Difference]) -> String {
    let cells: Vec<[String; 5]> = differences
        .iter()
        .map(|difference| {
            [
                difference.container.to_string(),
                difference.fingerprint.to_string(),
                difference.before.to_string(),
                "->".to_owned(),
                difference.after.to_string(),
            ]
        })
        .collect();
    table(&cells)
}

#[must_use]
pub fn sync(outcome: &SyncOutcome) -> String {
    match outcome {
        SyncOutcome::LockHeldElsewhere => "sync: another run already holds the lock".to_owned(),
        SyncOutcome::GameRunning => {
            "sync: skipped while Diablo II Resurrected is running".to_owned()
        }
        SyncOutcome::GameStartedDuringSync => {
            "sync: Diablo II Resurrected started during the mirror; commit deferred".to_owned()
        }
        SyncOutcome::NothingChanged => "sync: the mirror already matches the saves".to_owned(),
        SyncOutcome::Committed { commit, at } => format!("sync: committed {commit} for {at}"),
    }
}

#[must_use]
pub fn applied(applied: &Applied) -> String {
    let mut text = String::new();
    for step in &applied.realized {
        let _ = writeln!(text, "{}", realized(step));
    }
    let _ = writeln!(
        text,
        "apply: {} file(s) written, {} duplication(s) recorded",
        applied.written.len(),
        applied.duplications.len()
    );
    let _ = writeln!(text, "apply: return point {}", applied.checkpoint);
    let _ = write!(
        text,
        "{}",
        match &applied.record {
            Record::NothingChanged => "apply: the saves already held the outcome".to_owned(),
            Record::Committed { commit, at } => format!("apply: committed {commit} for {at}"),
        }
    );
    text
}

fn realized(step: &Realized) -> String {
    let verb = match step.transfer() {
        Transfer::Move => "move",
        Transfer::Duplicate => "duplicate",
    };
    let landing = match step.landing() {
        Landing::Cell(cell) => format!("@{},{}", cell.column().get(), cell.row().get()),
        Landing::Pile(units) => format!("x{units}"),
    };
    format!(
        "{verb} {} from {} to {} {landing}",
        step.fingerprint(),
        step.from(),
        step.to()
    )
}

fn table<const COLUMNS: usize>(cells: &[[String; COLUMNS]]) -> String {
    let mut widths = [0usize; COLUMNS];
    for row in cells {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut text = String::new();
    for row in cells {
        let mut line = String::new();
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                line.push_str("  ");
            }
            let _ = write!(line, "{cell:<width$}", width = widths[index]);
        }
        let _ = writeln!(text, "{}", line.trim_end());
    }
    text
}
