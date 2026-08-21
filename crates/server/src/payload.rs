use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use app::inspect::{Difference, Placement, Row};
use app::ledger::LedgerEntry;
use app::selector::ContainerSelector;
use app::sync::SyncOutcome;
use app::transaction::{Applied, PlanDocument, Record, TransferDocument};
use domain::plan::{Landing, Realized, Transfer};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncRequest {
    pub source: PathBuf,
    pub repo: PathBuf,
    pub lock: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SyncBody {
    LockHeldElsewhere,
    GameRunning,
    GameStartedDuringSync,
    NothingChanged,
    Committed { commit: String, at: String },
}

impl SyncBody {
    #[must_use]
    pub fn of(outcome: &SyncOutcome) -> Self {
        match outcome {
            SyncOutcome::LockHeldElsewhere => Self::LockHeldElsewhere,
            SyncOutcome::GameRunning => Self::GameRunning,
            SyncOutcome::GameStartedDuringSync => Self::GameStartedDuringSync,
            SyncOutcome::NothingChanged => Self::NothingChanged,
            SyncOutcome::Committed { commit, at } => Self::Committed {
                commit: commit.to_string(),
                at: at.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementBody {
    Cell { column: u8, row: u8 },
    Units(u8),
}

impl PlacementBody {
    #[must_use]
    pub const fn of(placement: Placement) -> Self {
        match placement {
            Placement::Cell(cell) => Self::Cell {
                column: cell.column().get(),
                row: cell.row().get(),
            },
            Placement::Units(count) => Self::Units(count.get()),
        }
    }

    #[must_use]
    pub const fn landed(landing: Landing) -> Self {
        match landing {
            Landing::Cell(cell) => Self::Cell {
                column: cell.column().get(),
                row: cell.row().get(),
            },
            Landing::Pile(units) => Self::Units(units.get()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemBody {
    pub container: String,
    pub placement: PlacementBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub quality: String,
    pub fingerprint: String,
}

impl ItemBody {
    #[must_use]
    pub fn of(row: &Row) -> Self {
        Self {
            container: row.container.to_string(),
            placement: PlacementBody::of(row.placement),
            code: row.code.map(|code| code.to_string()),
            quality: row.quality.to_string(),
            fingerprint: row.fingerprint.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemsBody {
    pub items: Vec<ItemBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DifferenceBody {
    pub container: String,
    pub fingerprint: String,
    pub before: u32,
    pub after: u32,
}

impl DifferenceBody {
    #[must_use]
    pub fn of(difference: &Difference) -> Self {
        Self {
            container: difference.container.to_string(),
            fingerprint: difference.fingerprint.to_string(),
            before: difference.before,
            after: difference.after,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffBody {
    pub differences: Vec<DifferenceBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {
    pub saves: PathBuf,
    pub plan: PlanDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanBody {
    pub plan: PlanDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    pub repo: PathBuf,
    #[serde(default)]
    pub saves: Option<PathBuf>,
    pub plan: PlanDocument,
    #[serde(default)]
    pub timer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RecordBody {
    NothingChanged,
    Committed { commit: String, at: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealizedBody {
    pub transfer: TransferDocument,
    pub fingerprint: String,
    pub from: String,
    pub to: String,
    pub landing: PlacementBody,
}

impl RealizedBody {
    #[must_use]
    pub fn of(step: &Realized) -> Self {
        Self {
            transfer: match step.transfer() {
                Transfer::Move => TransferDocument::Move,
                Transfer::Duplicate => TransferDocument::Duplicate,
            },
            fingerprint: step.fingerprint().to_string(),
            from: ContainerSelector::of(step.from()).to_string(),
            to: ContainerSelector::of(step.to()).to_string(),
            landing: PlacementBody::landed(step.landing()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyBody {
    pub checkpoint: String,
    pub record: RecordBody,
    pub realized: Vec<RealizedBody>,
    pub duplications: Vec<LedgerEntry>,
    pub written: Vec<String>,
}

impl ApplyBody {
    #[must_use]
    pub fn of(applied: &Applied) -> Self {
        Self {
            checkpoint: applied.checkpoint.to_string(),
            record: match &applied.record {
                Record::NothingChanged => RecordBody::NothingChanged,
                Record::Committed { commit, at } => RecordBody::Committed {
                    commit: commit.to_string(),
                    at: at.to_string(),
                },
            },
            realized: applied.realized.iter().map(RealizedBody::of).collect(),
            duplications: applied.duplications.clone(),
            written: applied
                .written
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        }
    }
}
