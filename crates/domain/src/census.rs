use std::collections::BTreeMap;

use crate::container::ContainerId;
use crate::error::ConservationError;
use crate::object::Fingerprint;
use crate::vault::{ContainerRef, Vault};

pub type Units = BTreeMap<Fingerprint, u32>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    containers: BTreeMap<ContainerId, Units>,
}

impl Census {
    #[must_use]
    pub fn of(vault: &Vault) -> Self {
        let mut containers: BTreeMap<ContainerId, Units> = BTreeMap::new();
        for (id, container) in vault.containers() {
            let units = containers.entry(id).or_default();
            match container {
                ContainerRef::Grid(grid) => {
                    for slot in grid.slots() {
                        *units.entry(slot.object().fingerprint()).or_default() += 1;
                    }
                }
                ContainerRef::Pile(pile) => {
                    for entry in pile.entries() {
                        *units.entry(entry.object().fingerprint()).or_default() +=
                            u32::from(entry.count().get());
                    }
                }
            }
        }
        Self { containers }
    }

    #[must_use]
    pub fn units(&self, container: &ContainerId) -> Option<&Units> {
        self.containers.get(container)
    }

    #[must_use]
    pub fn totals(&self) -> Units {
        let mut totals = Units::new();
        for units in self.containers.values() {
            for (fingerprint, count) in units {
                *totals.entry(*fingerprint).or_default() += count;
            }
        }
        totals
    }

    pub fn containers(&self) -> impl Iterator<Item = (&ContainerId, &Units)> {
        self.containers.iter()
    }

    pub fn conserved(&self, after: &Self, duplicated: &Units) -> Result<(), ConservationError> {
        let mut expected = self.totals();
        for (fingerprint, count) in duplicated {
            *expected.entry(*fingerprint).or_default() += count;
        }
        let observed = after.totals();

        let mut missing = Units::new();
        let mut appeared = Units::new();
        for (fingerprint, count) in &expected {
            let held = observed.get(fingerprint).copied().unwrap_or_default();
            if held < *count {
                missing.insert(*fingerprint, count - held);
            }
        }
        for (fingerprint, count) in &observed {
            let wanted = expected.get(fingerprint).copied().unwrap_or_default();
            if wanted < *count {
                appeared.insert(*fingerprint, count - wanted);
            }
        }
        if missing.is_empty() && appeared.is_empty() {
            Ok(())
        } else {
            Err(ConservationError { missing, appeared })
        }
    }
}
