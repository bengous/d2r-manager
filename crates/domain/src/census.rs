use std::collections::BTreeMap;

use crate::container::ContainerId;
use crate::object::Fingerprint;
use crate::vault::{ContainerRef, Vault};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    units: BTreeMap<(ContainerId, Fingerprint), u32>,
}

impl Census {
    #[must_use]
    pub fn of(vault: &Vault) -> Self {
        let mut units: BTreeMap<(ContainerId, Fingerprint), u32> = BTreeMap::new();
        for (id, container) in vault.containers() {
            match container {
                ContainerRef::Grid(grid) => {
                    for slot in grid.slots() {
                        *units
                            .entry((id.clone(), slot.object().fingerprint()))
                            .or_default() += 1;
                    }
                }
                ContainerRef::Pile(pile) => {
                    for entry in pile.entries() {
                        *units
                            .entry((id.clone(), entry.object().fingerprint()))
                            .or_default() += u32::from(entry.count().get());
                    }
                }
            }
        }
        Self { units }
    }

    #[must_use]
    pub fn units(&self, container: &ContainerId, fingerprint: Fingerprint) -> u32 {
        self.units
            .get(&(container.clone(), fingerprint))
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn totals(&self) -> BTreeMap<Fingerprint, u32> {
        let mut totals: BTreeMap<Fingerprint, u32> = BTreeMap::new();
        for ((_, fingerprint), count) in &self.units {
            *totals.entry(*fingerprint).or_default() += count;
        }
        totals
    }

    pub fn entries(&self) -> impl Iterator<Item = (&ContainerId, Fingerprint, u32)> {
        self.units
            .iter()
            .map(|((container, fingerprint), count)| (container, *fingerprint, *count))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conservation {
    Held {
        duplicated: BTreeMap<Fingerprint, u32>,
    },
    Broken {
        missing: BTreeMap<Fingerprint, u32>,
        appeared: BTreeMap<Fingerprint, u32>,
    },
}

impl Conservation {
    #[must_use]
    pub fn between(
        before: &Census,
        after: &Census,
        duplicated: &BTreeMap<Fingerprint, u32>,
    ) -> Self {
        let mut expected = before.totals();
        for (fingerprint, count) in duplicated {
            *expected.entry(*fingerprint).or_default() += count;
        }
        let observed = after.totals();

        let mut missing = BTreeMap::new();
        let mut appeared = BTreeMap::new();
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
            Self::Held {
                duplicated: duplicated.clone(),
            }
        } else {
            Self::Broken { missing, appeared }
        }
    }

    #[must_use]
    pub const fn is_held(&self) -> bool {
        matches!(self, Self::Held { .. })
    }
}
