use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use format::item::{Item, ItemBody, ItemPosition, SlotMode, StorePage};
use format::save::CharacterSave;
use format::stash::{StashSave, TabBody};

use crate::container::{
    CharacterName, CharacterPage, ContainerId, GridId, PileId, StashName, TabIndex,
};
use crate::error::VaultError;
use crate::gold::Gold;
use crate::grid::{Grid, GridPosition};
use crate::object::Object;
use crate::pile::{Pile, PileCount};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerRef<'a> {
    Grid(&'a Grid),
    Pile(&'a Pile),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vault {
    grids: BTreeMap<GridId, Grid>,
    piles: BTreeMap<PileId, Pile>,
    gold: BTreeMap<ContainerId, Gold>,
}

impl Vault {
    #[must_use]
    pub fn builder() -> VaultBuilder {
        VaultBuilder::default()
    }

    #[must_use]
    pub fn grid(&self, id: &GridId) -> Option<&Grid> {
        self.grids.get(id)
    }

    #[must_use]
    pub fn pile(&self, id: &PileId) -> Option<&Pile> {
        self.piles.get(id)
    }

    #[must_use]
    pub fn holds(&self, id: &ContainerId) -> bool {
        match id {
            ContainerId::Grid(grid) => self.grids.contains_key(grid),
            ContainerId::Pile(pile) => self.piles.contains_key(pile),
        }
    }

    #[must_use]
    pub fn gold(&self, id: &ContainerId) -> Option<Gold> {
        self.gold.get(id).copied()
    }

    pub fn grids(&self) -> impl Iterator<Item = (&GridId, &Grid)> {
        self.grids.iter()
    }

    pub fn piles(&self) -> impl Iterator<Item = (&PileId, &Pile)> {
        self.piles.iter()
    }

    pub fn containers(&self) -> impl Iterator<Item = (ContainerId, ContainerRef<'_>)> {
        self.grids
            .iter()
            .map(|(id, grid)| (ContainerId::Grid(id.clone()), ContainerRef::Grid(grid)))
            .chain(
                self.piles
                    .iter()
                    .map(|(id, pile)| (ContainerId::Pile(id.clone()), ContainerRef::Pile(pile))),
            )
    }

    pub(crate) fn grid_mut(&mut self, id: &GridId) -> Option<&mut Grid> {
        self.grids.get_mut(id)
    }

    pub(crate) fn pile_mut(&mut self, id: &PileId) -> Option<&mut Pile> {
        self.piles.get_mut(id)
    }
}

#[derive(Debug, Default)]
pub struct VaultBuilder {
    vault: Vault,
}

impl VaultBuilder {
    pub fn grid(
        mut self,
        id: &GridId,
        placements: Vec<(GridPosition, Object)>,
    ) -> Result<Self, VaultError> {
        let container = ContainerId::Grid(id.clone());
        let mut grid = Grid::new(id.geometry());
        for (at, object) in placements {
            grid.place(id, object, at)
                .map_err(|violation| VaultError::Placement {
                    container: container.clone(),
                    violation,
                })?;
        }
        match self.vault.grids.entry(id.clone()) {
            Entry::Occupied(_) => Err(VaultError::ContainerDeclaredTwice { container }),
            Entry::Vacant(free) => {
                free.insert(grid);
                Ok(self)
            }
        }
    }

    pub fn pile(
        mut self,
        id: &PileId,
        entries: Vec<(Object, PileCount)>,
    ) -> Result<Self, VaultError> {
        let mut pile = Pile::new();
        for (object, count) in entries {
            pile.restore(object, count);
        }
        match self.vault.piles.entry(id.clone()) {
            Entry::Occupied(_) => Err(VaultError::ContainerDeclaredTwice {
                container: ContainerId::Pile(id.clone()),
            }),
            Entry::Vacant(free) => {
                free.insert(pile);
                Ok(self)
            }
        }
    }

    #[must_use]
    pub fn with_gold(mut self, id: &ContainerId, gold: Gold) -> Self {
        self.vault.gold.insert(id.clone(), gold);
        self
    }

    pub fn shared_stash(self, stash: &StashName, save: &StashSave) -> Result<Self, VaultError> {
        let tabs = u8::try_from(save.tabs.len()).map_err(|_| VaultError::TooManyTabs {
            stash: stash.clone(),
            tabs: save.tabs.len(),
            max: u8::MAX,
        })?;
        let mut builder = self;
        for index in 0..tabs {
            let tab = &save.tabs[usize::from(index)];
            let tab_index = TabIndex::new(index);
            let id = match &tab.body {
                TabBody::Chronicle(_) => continue,
                TabBody::Normal(items) => {
                    let grid_id = GridId::SharedTab {
                        stash: stash.clone(),
                        tab: tab_index,
                    };
                    let container = ContainerId::Grid(grid_id.clone());
                    let placements = placements(&container, &items.0)?;
                    builder = builder.grid(&grid_id, placements)?;
                    container
                }
                TabBody::AdvancedStash(items) => {
                    let pile_id = PileId::new(stash.clone(), tab_index);
                    let container = ContainerId::Pile(pile_id.clone());
                    let entries = pile_entries(&container, &items.0)?;
                    builder = builder.pile(&pile_id, entries)?;
                    container
                }
            };
            let gold = Gold::new(tab.gold).map_err(|source| VaultError::Gold {
                container: id.clone(),
                source,
            })?;
            builder = builder.with_gold(&id, gold);
        }
        Ok(builder)
    }

    pub fn character(
        self,
        character: &CharacterName,
        save: &CharacterSave,
    ) -> Result<Self, VaultError> {
        let mut pages: BTreeMap<CharacterPage, Vec<Item>> = BTreeMap::new();
        for item in &save.items.0 {
            let ItemPosition::Slot {
                mode: SlotMode::Stored,
                page,
                ..
            } = item.position
            else {
                continue;
            };
            let Some(page) = character_page(page) else {
                continue;
            };
            pages.entry(page).or_default().push(item.clone());
        }
        let mut builder = self;
        for (page, items) in pages {
            let grid_id = GridId::Character {
                character: character.clone(),
                page,
            };
            let container = ContainerId::Grid(grid_id.clone());
            let placements = placements(&container, &items)?;
            builder = builder.grid(&grid_id, placements)?;
        }
        Ok(builder)
    }

    #[must_use]
    pub fn build(self) -> Vault {
        self.vault
    }
}

const fn character_page(page: StorePage) -> Option<CharacterPage> {
    match page {
        StorePage::Inventory => Some(CharacterPage::Inventory),
        StorePage::Stash => Some(CharacterPage::Stash),
        StorePage::Cube => Some(CharacterPage::Cube),
        StorePage::None
        | StorePage::Equip
        | StorePage::Trade
        | StorePage::Belt
        | StorePage::Seventh => None,
    }
}

fn placements(
    container: &ContainerId,
    items: &[Item],
) -> Result<Vec<(GridPosition, Object)>, VaultError> {
    items
        .iter()
        .map(|item| {
            let ItemPosition::Slot {
                mode: SlotMode::Stored,
                cell,
                ..
            } = item.position
            else {
                return Err(VaultError::NotStored {
                    container: container.clone(),
                });
            };
            let object = object(container, item)?;
            Ok((GridPosition::from_cell(cell), object))
        })
        .collect()
}

fn pile_entries(
    container: &ContainerId,
    items: &[Item],
) -> Result<Vec<(Object, PileCount)>, VaultError> {
    items
        .iter()
        .map(|item| {
            let count = PileCount::new(stack_of(item)).map_err(|source| VaultError::Pile {
                container: container.clone(),
                source,
            })?;
            Ok((object(container, item)?, count))
        })
        .collect()
}

fn object(container: &ContainerId, item: &Item) -> Result<Object, VaultError> {
    Object::new(item.clone()).map_err(|source| VaultError::Object {
        container: container.clone(),
        source,
    })
}

fn stack_of(item: &Item) -> u8 {
    let stack = match &item.body {
        ItemBody::Compact(body) => body.advanced_stash_stack,
        ItemBody::Complete(body) => body.advanced_stash_stack,
    };
    #[expect(clippy::cast_possible_truncation, reason = "a pile stack holds 8 bits")]
    let count = stack.map_or(0, |value| value.get() as u8);
    count
}
