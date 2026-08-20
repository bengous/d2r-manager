use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use format::item::{Item, ItemPosition, SlotMode, StorePage};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Container {
    Grid(Grid),
    Pile(Pile),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vault {
    containers: BTreeMap<ContainerId, Container>,
    gold: BTreeMap<ContainerId, Gold>,
}

impl Vault {
    #[must_use]
    pub fn builder() -> VaultBuilder {
        VaultBuilder::default()
    }

    #[must_use]
    pub fn container(&self, id: &ContainerId) -> Option<&Container> {
        self.containers.get(id)
    }

    #[must_use]
    pub fn grid(&self, id: &GridId) -> Option<&Grid> {
        match self.containers.get(&ContainerId::Grid(id.clone()))? {
            Container::Grid(grid) => Some(grid),
            Container::Pile(_) => None,
        }
    }

    #[must_use]
    pub fn pile(&self, id: &PileId) -> Option<&Pile> {
        match self.containers.get(&ContainerId::Pile(id.clone()))? {
            Container::Pile(pile) => Some(pile),
            Container::Grid(_) => None,
        }
    }

    #[must_use]
    pub fn gold(&self, id: &ContainerId) -> Option<Gold> {
        self.gold.get(id).copied()
    }

    pub fn containers(&self) -> impl Iterator<Item = (&ContainerId, &Container)> {
        self.containers.iter()
    }

    pub(crate) fn container_mut(&mut self, id: &ContainerId) -> Option<&mut Container> {
        self.containers.get_mut(id)
    }
}

#[derive(Debug, Default)]
pub struct VaultBuilder {
    vault: Vault,
}

impl VaultBuilder {
    pub fn shared_stash(mut self, stash: &StashName, save: &StashSave) -> Result<Self, VaultError> {
        let tabs = u8::try_from(save.tabs.len()).map_err(|_| VaultError::TooManyTabs {
            stash: stash.clone(),
            tabs: save.tabs.len(),
            max: u8::MAX,
        })?;
        for index in 0..tabs {
            let tab = &save.tabs[usize::from(index)];
            let tab_index = TabIndex::new(index);
            let (id, container) = match &tab.body {
                TabBody::Chronicle(_) => continue,
                TabBody::Normal(items) => {
                    let grid_id = GridId::SharedTab {
                        stash: stash.clone(),
                        tab: tab_index,
                    };
                    let id = ContainerId::Grid(grid_id.clone());
                    let grid = build_grid(&id, &grid_id, &items.0)?;
                    (id, Container::Grid(grid))
                }
                TabBody::AdvancedStash(items) => {
                    let id = ContainerId::Pile(PileId::new(stash.clone(), tab_index));
                    (id.clone(), Container::Pile(build_pile(&id, &items.0)?))
                }
            };
            let gold = Gold::new(tab.gold).map_err(|source| VaultError::Gold {
                container: id.clone(),
                source,
            })?;
            self.insert(id.clone(), container)?;
            self.vault.gold.insert(id, gold);
        }
        Ok(self)
    }

    pub fn character(
        mut self,
        character: &CharacterName,
        save: &CharacterSave,
    ) -> Result<Self, VaultError> {
        let mut pages: BTreeMap<CharacterPage, Vec<&Item>> = BTreeMap::new();
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
            pages.entry(page).or_default().push(item);
        }
        for (page, items) in pages {
            let grid_id = GridId::Character {
                character: character.clone(),
                page,
            };
            let id = ContainerId::Grid(grid_id.clone());
            let mut grid = Grid::new(grid_id.geometry());
            for item in items {
                place(&id, &grid_id, &mut grid, item)?;
            }
            self.insert(id, Container::Grid(grid))?;
        }
        Ok(self)
    }

    #[must_use]
    pub fn build(self) -> Vault {
        self.vault
    }

    fn insert(&mut self, id: ContainerId, container: Container) -> Result<(), VaultError> {
        match self.vault.containers.entry(id) {
            Entry::Occupied(taken) => Err(VaultError::ContainerDeclaredTwice {
                container: taken.key().clone(),
            }),
            Entry::Vacant(free) => {
                free.insert(container);
                Ok(())
            }
        }
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

fn build_grid(id: &ContainerId, grid_id: &GridId, items: &[Item]) -> Result<Grid, VaultError> {
    let mut grid = Grid::new(grid_id.geometry());
    for item in items {
        place(id, grid_id, &mut grid, item)?;
    }
    Ok(grid)
}

fn place(
    id: &ContainerId,
    grid_id: &GridId,
    grid: &mut Grid,
    item: &Item,
) -> Result<(), VaultError> {
    let ItemPosition::Slot {
        mode: SlotMode::Stored,
        cell,
        ..
    } = item.position
    else {
        return Err(VaultError::NotStored {
            container: id.clone(),
        });
    };
    let object = Object::new(item.clone()).map_err(|source| VaultError::Object {
        container: id.clone(),
        source,
    })?;
    grid.place(grid_id, object, GridPosition::from_cell(cell))
        .map_err(|violation| VaultError::Placement {
            container: id.clone(),
            violation,
        })
}

fn build_pile(id: &ContainerId, items: &[Item]) -> Result<Pile, VaultError> {
    let mut pile = Pile::new();
    for item in items {
        let object = Object::new(item.clone()).map_err(|source| VaultError::Object {
            container: id.clone(),
            source,
        })?;
        let count = PileCount::new(stack_of(item)).map_err(|source| VaultError::Pile {
            container: id.clone(),
            source,
        })?;
        pile.restore(object, count);
    }
    Ok(pile)
}

fn stack_of(item: &Item) -> u8 {
    let stack = match &item.body {
        format::item::ItemBody::Compact(body) => body.advanced_stash_stack,
        format::item::ItemBody::Complete(body) => body.advanced_stash_stack,
    };
    #[expect(clippy::cast_possible_truncation, reason = "a pile stack holds 8 bits")]
    let count = stack.map_or(0, |value| value.get() as u8);
    count
}
