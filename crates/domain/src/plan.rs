use std::collections::BTreeMap;

use crate::census::{Census, Conservation};
use crate::container::{CharacterName, ContainerId, GridId, PileId};
use crate::error::{ApplyError, PileError, PlanError, RuleViolation};
use crate::gold::Gold;
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
        let mut duplicated: BTreeMap<Fingerprint, u32> = BTreeMap::new();

        for (index, operation) in self.operations.iter().enumerate() {
            let step =
                run(&mut outcome, operation).map_err(|violation| PlanError { index, violation })?;
            if operation.transfer == Transfer::Duplicate {
                *duplicated.entry(step.fingerprint).or_default() +=
                    u32::from(operation.from.units().get());
            }
            realized.push(step);
        }

        let after = Census::of(&outcome);
        let conservation = Conservation::between(&before, &after, &duplicated);
        Ok(ValidatedPlan {
            operations: self.operations,
            realized,
            outcome,
            conservation,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedPlan {
    operations: Vec<Operation>,
    realized: Vec<Realized>,
    outcome: Vault,
    conservation: Conservation,
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
    pub const fn conservation(&self) -> &Conservation {
        &self.conservation
    }

    pub fn apply(self, observed: &Vault) -> Result<AppliedPlan, ApplyError> {
        let expected = Census::of(&self.outcome);
        let seen = Census::of(observed);

        for (container, fingerprint, count) in expected.entries() {
            if !observed.holds(container) {
                return Err(ApplyError::ContainerMissing {
                    container: container.clone(),
                });
            }
            let held = seen.units(container, fingerprint);
            if held != count {
                return Err(ApplyError::UnitsDiffer {
                    container: container.clone(),
                    fingerprint,
                    expected: count,
                    observed: held,
                });
            }
        }
        for (container, fingerprint, count) in seen.entries() {
            if !self.outcome.holds(container) {
                return Err(ApplyError::ContainerUnexpected {
                    container: container.clone(),
                });
            }
            let wanted = expected.units(container, fingerprint);
            if wanted != count {
                return Err(ApplyError::UnitsDiffer {
                    container: container.clone(),
                    fingerprint,
                    expected: wanted,
                    observed: count,
                });
            }
        }
        for (container, _) in self.outcome.containers() {
            let wanted = self.outcome.gold(&container);
            let held = observed.gold(&container);
            if wanted != held {
                return Err(ApplyError::GoldDiffers {
                    container,
                    expected: wanted.unwrap_or(Gold::ZERO),
                    observed: held.unwrap_or(Gold::ZERO),
                });
            }
        }

        Ok(AppliedPlan {
            realized: self.realized,
            conservation: self.conservation,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AppliedPlan {
    realized: Vec<Realized>,
    conservation: Conservation,
}

impl AppliedPlan {
    #[must_use]
    pub fn realized(&self) -> &[Realized] {
        &self.realized
    }

    #[must_use]
    pub const fn conservation(&self) -> &Conservation {
        &self.conservation
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
