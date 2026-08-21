use serde::{Deserialize, Serialize};

use domain::plan::{Landing, Realized, Transfer};

use crate::selector::ContainerSelector;
use crate::value::Timestamp;

pub const LEDGER_DIRECTORY: &str = "ledger";
pub const LEDGER_FILE: &str = "duplications.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LedgerEntry {
    pub at: String,
    pub fingerprint: String,
    pub from: String,
    pub to: String,
    pub units: u8,
}

impl LedgerEntry {
    fn of(step: &Realized, at: &Timestamp) -> Self {
        Self {
            at: at.to_string(),
            fingerprint: step.fingerprint().to_string(),
            from: ContainerSelector::of(step.from()).to_string(),
            to: ContainerSelector::of(step.to()).to_string(),
            units: match step.landing() {
                Landing::Cell(_) => 1,
                Landing::Pile(units) => units.get(),
            },
        }
    }
}

#[must_use]
pub fn duplications(realized: &[Realized], at: &Timestamp) -> Vec<LedgerEntry> {
    realized
        .iter()
        .filter(|step| step.transfer() == Transfer::Duplicate)
        .map(|step| LedgerEntry::of(step, at))
        .collect()
}
