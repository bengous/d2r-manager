use std::collections::BTreeSet;

use crate::census::{Census, Units};
use crate::container::{CharacterName, ContainerId, GridId, PileId};
use crate::error::{ApplyError, ConservationError, PileError, PlanError, RuleViolation};
use crate::grid::{Grid, GridPosition};
use crate::object::{Fingerprint, Object};
use crate::pile::{Pile, PileUnits};
use crate::tables::CarryGroup;
use crate::vault::Vault;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Grid {
        container: GridId,
        fingerprint: Fingerprint,
    },
    Pile {
        container: PileId,
        fingerprint: Fingerprint,
        units: PileUnits,
    },
}

impl Source {
    #[must_use]
    pub fn container(&self) -> ContainerId {
        match self {
            Self::Grid { container, .. } => ContainerId::Grid(container.clone()),
            Self::Pile { container, .. } => ContainerId::Pile(container.clone()),
        }
    }

    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::Grid { fingerprint, .. } | Self::Pile { fingerprint, .. } => *fingerprint,
        }
    }

    #[must_use]
    pub const fn units(&self) -> PileUnits {
        match self {
            Self::Grid { .. } => PileUnits::ONE,
            Self::Pile { units, .. } => *units,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    Grid {
        container: GridId,
        cell: Option<GridPosition>,
    },
    Pile {
        container: PileId,
    },
}

impl Destination {
    #[must_use]
    pub fn container(&self) -> ContainerId {
        match self {
            Self::Grid { container, .. } => ContainerId::Grid(container.clone()),
            Self::Pile { container } => ContainerId::Pile(container.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    Move,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    transfer: Transfer,
    from: Source,
    to: Destination,
}

impl Operation {
    #[must_use]
    pub const fn moved(from: Source, to: Destination) -> Self {
        Self {
            transfer: Transfer::Move,
            from,
            to,
        }
    }

    #[must_use]
    pub const fn duplicated(from: Source, to: Destination) -> Self {
        Self {
            transfer: Transfer::Duplicate,
            from,
            to,
        }
    }

    #[must_use]
    pub const fn transfer(&self) -> Transfer {
        self.transfer
    }

    #[must_use]
    pub const fn from(&self) -> &Source {
        &self.from
    }

    #[must_use]
    pub const fn to(&self) -> &Destination {
        &self.to
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    Cell(GridPosition),
    Pile(PileUnits),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realized {
    transfer: Transfer,
    fingerprint: Fingerprint,
    from: ContainerId,
    to: ContainerId,
    landing: Landing,
}

impl Realized {
    #[must_use]
    pub const fn transfer(&self) -> Transfer {
        self.transfer
    }

    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    #[must_use]
    pub const fn from(&self) -> &ContainerId {
        &self.from
    }

    #[must_use]
    pub const fn to(&self) -> &ContainerId {
        &self.to
    }

    #[must_use]
    pub const fn landing(&self) -> Landing {
        self.landing
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    operations: Vec<Operation>,
}

impl Plan {
    #[must_use]
    pub const fn new(operations: Vec<Operation>) -> Self {
        Self { operations }
    }

    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn validate(self, vault: Vault) -> Result<ValidatedPlan, PlanError> {
        let before = Census::of(&vault);
        let mut outcome = vault;
        let mut realized = Vec::with_capacity(self.operations.len());
        let mut duplicated = Units::new();

        for (index, operation) in self.operations.iter().enumerate() {
            let step =
                run(&mut outcome, operation).map_err(|violation| PlanError { index, violation })?;
            if operation.transfer == Transfer::Duplicate {
                *duplicated.entry(step.fingerprint).or_default() +=
                    u32::from(operation.from.units().get());
            }
            realized.push(step);
        }

        Ok(ValidatedPlan {
            operations: self.operations,
            realized,
            before,
            outcome,
            duplicated,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedPlan {
    operations: Vec<Operation>,
    realized: Vec<Realized>,
    before: Census,
    outcome: Vault,
    duplicated: Units,
}

impl ValidatedPlan {
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    #[must_use]
    pub fn realized(&self) -> &[Realized] {
        &self.realized
    }

    #[must_use]
    pub const fn outcome(&self) -> &Vault {
        &self.outcome
    }

    #[must_use]
    pub fn into_outcome(self) -> Vault {
        self.outcome
    }

    #[must_use]
    pub const fn duplicated(&self) -> &Units {
        &self.duplicated
    }

    pub fn conserved(&self) -> Result<(), ConservationError> {
        self.before
            .conserved(&Census::of(&self.outcome), &self.duplicated)
    }

    pub fn apply(self, observed: &Vault) -> Result<AppliedPlan, ApplyError> {
        let wanted: BTreeSet<ContainerId> = self.outcome.containers().map(|(id, _)| id).collect();
        let seen: BTreeSet<ContainerId> = observed.containers().map(|(id, _)| id).collect();
        if let Some(container) = wanted.difference(&seen).next() {
            return Err(ApplyError::ContainerMissing {
                container: container.clone(),
            });
        }
        if let Some(container) = seen.difference(&wanted).next() {
            return Err(ApplyError::ContainerUnexpected {
                container: container.clone(),
            });
        }

        let expected = Census::of(&self.outcome);
        let read_back = Census::of(observed);
        let empty = Units::new();
        for container in &wanted {
            let wanted_units = expected.units(container).unwrap_or(&empty);
            let held_units = read_back.units(container).unwrap_or(&empty);
            let fingerprints: BTreeSet<Fingerprint> = wanted_units
                .keys()
                .chain(held_units.keys())
                .copied()
                .collect();
            for fingerprint in fingerprints {
                let expected_count = wanted_units.get(&fingerprint).copied().unwrap_or_default();
                let observed_count = held_units.get(&fingerprint).copied().unwrap_or_default();
                if expected_count != observed_count {
                    return Err(ApplyError::UnitsDiffer {
                        container: container.clone(),
                        fingerprint,
                        expected: expected_count,
                        observed: observed_count,
                    });
                }
            }
        }
        self.before.conserved(&read_back, &self.duplicated)?;

        Ok(AppliedPlan {
            realized: self.realized,
            duplicated: self.duplicated,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AppliedPlan {
    realized: Vec<Realized>,
    duplicated: Units,
}

impl AppliedPlan {
    #[must_use]
    pub fn realized(&self) -> &[Realized] {
        &self.realized
    }

    #[must_use]
    pub const fn duplicated(&self) -> &Units {
        &self.duplicated
    }
}

fn run(vault: &mut Vault, operation: &Operation) -> Result<Realized, RuleViolation> {
    let from = operation.from.container();
    let to = operation.to.container();
    let object = take(vault, &operation.from, operation.transfer)?;
    let fingerprint = object.fingerprint();
    let carry = object.carry_group();
    let landing = put(vault, &operation.to, object, operation.from.units())?;

    if let (Some(character), Some(group)) = (to.character(), carry) {
        let held = carried(vault, character, group);
        if held > 1 {
            return Err(RuleViolation::CarryLimitExceeded {
                character: character.clone(),
                group,
                count: held,
            });
        }
    }

    Ok(Realized {
        transfer: operation.transfer,
        fingerprint,
        from,
        to,
        landing,
    })
}

fn take(vault: &mut Vault, source: &Source, transfer: Transfer) -> Result<Object, RuleViolation> {
    let id = source.container();
    match source {
        Source::Grid {
            container,
            fingerprint,
        } => {
            let grid = grid_mut(vault, container)?;
            match transfer {
                Transfer::Move => grid.take(*fingerprint),
                Transfer::Duplicate => grid.find(*fingerprint).cloned(),
            }
            .ok_or(RuleViolation::ObjectAbsent {
                container: id.clone(),
                fingerprint: *fingerprint,
            })
        }
        Source::Pile {
            container,
            fingerprint,
            units,
        } => {
            let pile = pile_mut(vault, container)?;
            let held = pile.units(*fingerprint);
            match transfer {
                Transfer::Move => pile
                    .take(*fingerprint, *units)
                    .map_err(|error| match error {
                        PileError::NotEnoughUnits { held, asked, .. } => {
                            RuleViolation::NotEnoughUnits {
                                container: id.clone(),
                                fingerprint: *fingerprint,
                                held,
                                asked,
                            }
                        }
                        PileError::Value(_) => RuleViolation::ObjectAbsent {
                            container: id.clone(),
                            fingerprint: *fingerprint,
                        },
                    }),
                Transfer::Duplicate => {
                    if held < u16::from(units.get()) {
                        return Err(RuleViolation::NotEnoughUnits {
                            container: id.clone(),
                            fingerprint: *fingerprint,
                            held,
                            asked: *units,
                        });
                    }
                    pile.find(*fingerprint)
                        .cloned()
                        .ok_or(RuleViolation::ObjectAbsent {
                            container: id.clone(),
                            fingerprint: *fingerprint,
                        })
                }
            }
        }
    }
}

fn put(
    vault: &mut Vault,
    destination: &Destination,
    object: Object,
    units: PileUnits,
) -> Result<Landing, RuleViolation> {
    match destination {
        Destination::Grid { container, cell } => {
            if units.get() > 1 {
                return Err(RuleViolation::GridTakesOneUnit {
                    container: container.clone(),
                    asked: units,
                });
            }
            let grid = grid_mut(vault, container)?;
            match cell {
                Some(cell) => {
                    grid.place(container, object, *cell)?;
                    Ok(Landing::Cell(*cell))
                }
                None => grid.append(container, object).map(Landing::Cell),
            }
        }
        Destination::Pile { container } => {
            let pile = pile_mut(vault, container)?;
            pile.add(object, units);
            Ok(Landing::Pile(units))
        }
    }
}

fn grid_mut<'a>(vault: &'a mut Vault, id: &GridId) -> Result<&'a mut Grid, RuleViolation> {
    vault
        .grid_mut(id)
        .ok_or_else(|| RuleViolation::UnknownContainer {
            container: ContainerId::Grid(id.clone()),
        })
}

fn pile_mut<'a>(vault: &'a mut Vault, id: &PileId) -> Result<&'a mut Pile, RuleViolation> {
    vault
        .pile_mut(id)
        .ok_or_else(|| RuleViolation::UnknownContainer {
            container: ContainerId::Pile(id.clone()),
        })
}

fn carried(vault: &Vault, character: &CharacterName, group: CarryGroup) -> u16 {
    let held: usize = vault
        .grids()
        .filter(|(id, _)| id.character() == Some(character))
        .map(|(_, grid)| {
            grid.slots()
                .iter()
                .filter(|slot| slot.object().carry_group() == Some(group))
                .count()
        })
        .sum();
    u16::try_from(held).unwrap_or(u16::MAX)
}
