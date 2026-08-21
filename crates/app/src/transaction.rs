use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use domain::error::{ApplyError, ModelError, PlanError, ValueError};
use domain::grid::{Column, GridPosition, Row};
use domain::object::Fingerprint;
use domain::pile::PileUnits;
use domain::plan::{Destination, Operation, Plan, Realized, Source, Transfer, ValidatedPlan};
use domain::vault::Vault;

use crate::ledger::{LEDGER_DIRECTORY, LEDGER_FILE, LedgerEntry, duplications};
use crate::ports::{
    Clock, CommandError, FileError, Files, PathState, Processes, Repository, Staged, TimerState,
    Timers,
};
use crate::selector::{ContainerSelector, SelectorError};
use crate::sync::{GameState, game};
use crate::value::{CommitId, Timestamp, UnitName};
use crate::world::{SaveTree, WorldError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferDocument {
    Move,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellDocument {
    pub column: u8,
    pub row: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDocument {
    pub container: String,
    pub fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationDocument {
    pub container: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<CellDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationDocument {
    pub transfer: TransferDocument,
    pub from: SourceDocument,
    pub to: DestinationDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanDocument {
    pub operations: Vec<OperationDocument>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResolveError {
    #[error("operation {index}: {source}")]
    Selector {
        index: usize,
        #[source]
        source: SelectorError,
    },

    #[error("operation {index}: fingerprint: {source}")]
    Fingerprint {
        index: usize,
        #[source]
        source: ValueError,
    },

    #[error("operation {index}: units: {source}")]
    Units {
        index: usize,
        #[source]
        source: ValueError,
    },

    #[error("operation {index}: cell: {source}")]
    Cell {
        index: usize,
        #[source]
        source: ModelError,
    },

    #[error("operation {index}: {container} is absent from the saves")]
    UnknownContainer {
        index: usize,
        container: ContainerSelector,
    },

    #[error(
        "operation {index}: {container} is a grid: it moves one item, so it takes no unit count"
    )]
    GridSourceUnits {
        index: usize,
        container: ContainerSelector,
    },

    #[error(
        "operation {index}: {container} is an advanced stash tab: items stack there, so it takes no cell"
    )]
    PileDestinationCell {
        index: usize,
        container: ContainerSelector,
    },
}

pub fn resolve(document: &PlanDocument, vault: &Vault) -> Result<Plan, ResolveError> {
    let mut operations = Vec::with_capacity(document.operations.len());
    for (index, operation) in document.operations.iter().enumerate() {
        let from = source(index, &operation.from, vault)?;
        let to = destination(index, &operation.to, vault)?;
        operations.push(match operation.transfer {
            TransferDocument::Move => Operation::moved(from, to),
            TransferDocument::Duplicate => Operation::duplicated(from, to),
        });
    }
    Ok(Plan::new(operations))
}

#[must_use]
pub fn describe(plan: &Plan) -> PlanDocument {
    PlanDocument {
        operations: plan.operations().iter().map(operation).collect(),
    }
}

pub const DEFAULT_TIMER: &str = "d2r-backup.timer";

const CHECKPOINT_PREFIX: &str = "checkpoint";
const APPLY_PREFIX: &str = "apply";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionPaths {
    pub repo: PathBuf,
    pub saves: PathBuf,
    pub timer: UnitName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    NothingChanged,
    Committed { commit: CommitId, at: Timestamp },
}

#[derive(Debug)]
pub struct Applied {
    pub checkpoint: CommitId,
    pub record: Record,
    pub realized: Vec<Realized>,
    pub duplications: Vec<LedgerEntry>,
    pub written: Vec<PathBuf>,
}

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("Diablo II Resurrected is running: close the game before a plan is applied")]
    GameRunning,

    #[error("unexpected repository root: {found}")]
    UnexpectedRoot { found: PathBuf },

    #[error("the saves at {saves} are not inside the repository at {root}")]
    SavesOutsideRepository { saves: PathBuf, root: PathBuf },

    #[error("the ledger path carries no directory: {path}")]
    LedgerWithoutDirectory { path: PathBuf },

    #[error(transparent)]
    Resolve(#[from] ResolveError),

    #[error("the plan holds an operation the rules refuse: {0}")]
    Refused(#[from] PlanError),

    #[error("the saves read back after writing hold another world than the plan: {0}")]
    ReadBack(#[from] ApplyError),

    #[error(transparent)]
    World(#[from] WorldError),

    #[error(transparent)]
    File(#[from] FileError),

    #[error(transparent)]
    Command(#[from] CommandError),

    #[error("the plan does not serialize: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Clock(#[from] crate::value::ValueError),

    #[error("the backup timer stayed paused: {source}")]
    TimerLeftPaused {
        #[source]
        source: CommandError,
    },

    #[error("{first}; and {second}")]
    Compound {
        first: Box<TransactionError>,
        second: Box<TransactionError>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pause {
    Paused,
    LeftAlone,
}

#[derive(Debug, Clone, Copy)]
struct Written<'a> {
    saves: &'a Path,
    ledger: &'a Path,
}

#[derive(Debug)]
pub struct Transaction<'a> {
    pub files: &'a dyn Files,
    pub processes: &'a dyn Processes,
    pub repository: &'a dyn Repository,
    pub timers: &'a dyn Timers,
    pub clock: &'a dyn Clock,
}

impl Transaction<'_> {
    pub fn apply(
        &self,
        paths: &TransactionPaths,
        document: &PlanDocument,
    ) -> Result<Applied, TransactionError> {
        if game(self.processes)? == GameState::Running {
            return Err(TransactionError::GameRunning);
        }

        let root = self.files.real_path(&self.repository.root(&paths.repo)?)?;
        if root != self.files.real_path(&paths.repo)? {
            return Err(TransactionError::UnexpectedRoot { found: root });
        }
        let saves = self.files.real_path(&paths.saves)?;
        let inside = saves
            .strip_prefix(&root)
            .ok()
            .filter(|relative| relative.components().next().is_some())
            .ok_or_else(|| TransactionError::SavesOutsideRepository {
                saves: saves.clone(),
                root: root.clone(),
            })?
            .to_owned();
        let ledger = root.join(LEDGER_DIRECTORY).join(LEDGER_FILE);

        let mut tree = SaveTree::read(self.files, &saves)?;
        let vault = tree.vault()?;
        let validated = resolve(document, &vault)?.validate(vault)?;
        tree.project(validated.outcome())?;

        let pause = self.pause(&paths.timer)?;
        let outcome = self.write(
            &paths.repo,
            Written {
                saves: &inside,
                ledger: &ledger,
            },
            &mut tree,
            validated,
            document,
        );
        self.resume(&paths.timer, pause, outcome)
    }

    fn write(
        &self,
        repo: &Path,
        target: Written<'_>,
        tree: &mut SaveTree,
        validated: ValidatedPlan,
        document: &PlanDocument,
    ) -> Result<Applied, TransactionError> {
        let Written { saves, ledger } = target;
        let checkpoint = self.checkpoint(repo, &self.specs(target)?)?;
        let written = tree.flush(self.files)?;

        let verified = SaveTree::read(self.files, tree.directory())
            .map_err(TransactionError::World)
            .and_then(|world| world.vault().map_err(TransactionError::World))
            .and_then(|world| validated.apply(&world).map_err(TransactionError::ReadBack));
        let applied = match verified {
            Ok(applied) => applied,
            Err(error) => return Err(self.rollback(repo, saves, &checkpoint, error)),
        };

        let at = self.clock.now()?;
        let duplications = duplications(applied.realized(), &at);
        if !duplications.is_empty() {
            let directory =
                ledger
                    .parent()
                    .ok_or_else(|| TransactionError::LedgerWithoutDirectory {
                        path: ledger.to_owned(),
                    })?;
            self.files.create_directory(directory)?;
            for entry in &duplications {
                self.files
                    .append_line(ledger, &serde_json::to_string(entry)?)?;
            }
        }

        let specs = self.specs(target)?;
        self.repository.stage(repo, &specs)?;
        let record = if self.repository.staged(repo, &specs)? == Staged::Nothing {
            Record::NothingChanged
        } else {
            let message = format!(
                "{APPLY_PREFIX}: {at}\n\n{}",
                serde_json::to_string_pretty(document)?
            );
            Record::Committed {
                commit: self.repository.commit(repo, &message, &specs)?,
                at,
            }
        };

        Ok(Applied {
            checkpoint,
            record,
            realized: applied.realized().to_vec(),
            duplications,
            written,
        })
    }

    fn specs<'a>(&self, target: Written<'a>) -> Result<Vec<&'a Path>, TransactionError> {
        let mut specs = vec![target.saves];
        if self.files.probe(target.ledger)? != PathState::Missing {
            specs.push(Path::new(LEDGER_DIRECTORY));
        }
        Ok(specs)
    }

    fn checkpoint(&self, repo: &Path, specs: &[&Path]) -> Result<CommitId, TransactionError> {
        self.repository.stage(repo, specs)?;
        if self.repository.staged(repo, specs)? == Staged::Changes {
            let at = self.clock.now()?;
            self.repository
                .commit(repo, &format!("{CHECKPOINT_PREFIX}: {at}"), specs)?;
        }
        Ok(self.repository.head(repo)?)
    }

    fn rollback(
        &self,
        repo: &Path,
        saves: &Path,
        checkpoint: &CommitId,
        error: TransactionError,
    ) -> TransactionError {
        match self.repository.restore(repo, checkpoint, &[saves]) {
            Ok(()) => error,
            Err(source) => TransactionError::Compound {
                first: Box::new(error),
                second: Box::new(TransactionError::Command(source)),
            },
        }
    }

    fn pause(&self, unit: &UnitName) -> Result<Pause, TransactionError> {
        match self.timers.state(unit)? {
            TimerState::Active => {
                self.timers.pause(unit)?;
                Ok(Pause::Paused)
            }
            TimerState::Inactive | TimerState::Absent => Ok(Pause::LeftAlone),
        }
    }

    fn resume(
        &self,
        unit: &UnitName,
        pause: Pause,
        outcome: Result<Applied, TransactionError>,
    ) -> Result<Applied, TransactionError> {
        if pause == Pause::LeftAlone {
            return outcome;
        }
        match (outcome, self.timers.resume(unit)) {
            (result, Ok(())) => result,
            (Ok(_), Err(source)) => Err(TransactionError::TimerLeftPaused { source }),
            (Err(first), Err(source)) => Err(TransactionError::Compound {
                first: Box::new(first),
                second: Box::new(TransactionError::TimerLeftPaused { source }),
            }),
        }
    }
}

fn source(index: usize, document: &SourceDocument, vault: &Vault) -> Result<Source, ResolveError> {
    let selector = selector(index, &document.container)?;
    let fingerprint = Fingerprint::parse(&document.fingerprint)
        .map_err(|source| ResolveError::Fingerprint { index, source })?;
    let grid = selector.grid();
    if vault.grid(&grid).is_some() {
        if document.units.is_some() {
            return Err(ResolveError::GridSourceUnits {
                index,
                container: selector,
            });
        }
        return Ok(Source::Grid {
            container: grid,
            fingerprint,
        });
    }
    let pile = selector
        .pile()
        .filter(|pile| vault.pile(pile).is_some())
        .ok_or_else(|| ResolveError::UnknownContainer {
            index,
            container: selector.clone(),
        })?;
    Ok(Source::Pile {
        container: pile,
        fingerprint,
        units: units(index, document.units)?,
    })
}

fn destination(
    index: usize,
    document: &DestinationDocument,
    vault: &Vault,
) -> Result<Destination, ResolveError> {
    let selector = selector(index, &document.container)?;
    let grid = selector.grid();
    if vault.grid(&grid).is_some() {
        return Ok(Destination::Grid {
            container: grid,
            cell: document
                .cell
                .map(|cell| {
                    GridPosition::new(Column::new(cell.column), Row::new(cell.row))
                        .map_err(|source| ResolveError::Cell { index, source })
                })
                .transpose()?,
        });
    }
    let pile = selector
        .pile()
        .filter(|pile| vault.pile(pile).is_some())
        .ok_or_else(|| ResolveError::UnknownContainer {
            index,
            container: selector.clone(),
        })?;
    if document.cell.is_some() {
        return Err(ResolveError::PileDestinationCell {
            index,
            container: selector,
        });
    }
    Ok(Destination::Pile { container: pile })
}

fn selector(index: usize, text: &str) -> Result<ContainerSelector, ResolveError> {
    ContainerSelector::from_str(text).map_err(|source| ResolveError::Selector { index, source })
}

fn units(index: usize, asked: Option<u8>) -> Result<PileUnits, ResolveError> {
    match asked {
        None => Ok(PileUnits::ONE),
        Some(units) => {
            PileUnits::new(units).map_err(|source| ResolveError::Units { index, source })
        }
    }
}

fn operation(operation: &Operation) -> OperationDocument {
    OperationDocument {
        transfer: match operation.transfer() {
            Transfer::Move => TransferDocument::Move,
            Transfer::Duplicate => TransferDocument::Duplicate,
        },
        from: SourceDocument {
            container: ContainerSelector::of(&operation.from().container()).to_string(),
            fingerprint: operation.from().fingerprint().to_string(),
            units: match operation.from() {
                Source::Grid { .. } => None,
                Source::Pile { units, .. } => Some(units.get()),
            },
        },
        to: DestinationDocument {
            container: ContainerSelector::of(&operation.to().container()).to_string(),
            cell: match operation.to() {
                Destination::Grid { cell, .. } => cell.map(|cell| CellDocument {
                    column: cell.column().get(),
                    row: cell.row().get(),
                }),
                Destination::Pile { .. } => None,
            },
        },
    }
}
