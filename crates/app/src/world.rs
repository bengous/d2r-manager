use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use thiserror::Error;

use domain::container::{
    CharacterName, CharacterPage, ContainerId, GridId, PileId, StashName, TabIndex,
};
use domain::error::{ModelError, ValueError, VaultError};
use domain::grid::{Grid, GridPosition};
use domain::object::Fingerprint;
use domain::pile::{Pile, PileCount};
use domain::vault::Vault;
use format::bits::Bits;
use format::error::FormatError;
use format::item::{BodyLocation, Item, ItemBody, ItemPosition, SlotMode, StorePage};
use format::save::{CharacterSave, ItemList};
use format::stash::{StashSave, TabBody};

use crate::ports::{FileError, Files, PathState};

pub const STASH_EXTENSION: &str = "d2i";
pub const CHARACTER_EXTENSION: &str = "d2s";
pub const CHARACTER_PAGES: [CharacterPage; 3] = [
    CharacterPage::Inventory,
    CharacterPage::Stash,
    CharacterPage::Cube,
];

#[derive(Debug, Error)]
pub enum WorldError {
    #[error(transparent)]
    File(#[from] FileError),

    #[error("{path} is not a directory of save files")]
    NotADirectory { path: PathBuf },

    #[error("{path} carries no name a container can take")]
    Unnamed { path: PathBuf },

    #[error("{path}: {source}")]
    Name {
        path: PathBuf,
        #[source]
        source: ValueError,
    },

    #[error("{path}: {source}")]
    Bitstream {
        path: PathBuf,
        #[source]
        source: FormatError,
    },

    #[error(transparent)]
    Vault(#[from] VaultError),

    #[error(transparent)]
    Model(#[from] ModelError),

    #[error("{container} is absent from the world the plan describes")]
    UnknownContainer { container: ContainerId },

    #[error(
        "{container}: the item {fingerprint} carries no advanced stash stack field, so no unit count fits in it"
    )]
    StackAbsent {
        container: ContainerId,
        fingerprint: Fingerprint,
    },

    #[error("{container}: {count} units overflow the 8 bits an advanced stash stack holds")]
    StackTooLarge { container: ContainerId, count: u8 },

    #[error("{path}: a stash holds at most {max} tabs, this one declares {tabs}")]
    TooManyTabs { path: PathBuf, tabs: usize, max: u8 },
}

#[derive(Debug)]
pub struct StashFile {
    name: StashName,
    path: PathBuf,
    bytes: Vec<u8>,
    save: StashSave,
}

#[derive(Debug)]
pub struct CharacterFile {
    name: CharacterName,
    path: PathBuf,
    bytes: Vec<u8>,
    save: CharacterSave,
}

#[derive(Debug)]
pub struct SaveTree {
    directory: PathBuf,
    stashes: Vec<StashFile>,
    characters: Vec<CharacterFile>,
}

impl SaveTree {
    pub fn read(files: &dyn Files, directory: &Path) -> Result<Self, WorldError> {
        if files.probe(directory)? != PathState::Directory {
            return Err(WorldError::NotADirectory {
                path: directory.to_owned(),
            });
        }
        let mut stashes = Vec::new();
        let mut characters = Vec::new();
        for path in files.entries(directory)? {
            let extension = path.extension().and_then(OsStr::to_str);
            if !matches!(extension, Some(STASH_EXTENSION | CHARACTER_EXTENSION)) {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(OsStr::to_str)
                .ok_or_else(|| WorldError::Unnamed { path: path.clone() })?;
            let bytes = files.read(&path)?;
            if extension == Some(STASH_EXTENSION) {
                stashes.push(StashFile {
                    name: StashName::new(stem).map_err(|source| WorldError::Name {
                        path: path.clone(),
                        source,
                    })?,
                    save: StashSave::read(&bytes).map_err(|source| WorldError::Bitstream {
                        path: path.clone(),
                        source,
                    })?,
                    path,
                    bytes,
                });
            } else {
                characters.push(CharacterFile {
                    name: CharacterName::new(stem).map_err(|source| WorldError::Name {
                        path: path.clone(),
                        source,
                    })?,
                    save: CharacterSave::read(&bytes).map_err(|source| WorldError::Bitstream {
                        path: path.clone(),
                        source,
                    })?,
                    path,
                    bytes,
                });
            }
        }
        Ok(Self {
            directory: directory.to_owned(),
            stashes,
            characters,
        })
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn vault(&self) -> Result<Vault, WorldError> {
        let mut builder = Vault::builder();
        for stash in &self.stashes {
            builder = builder.shared_stash(&stash.name, &stash.save)?;
        }
        for character in &self.characters {
            let filled = character.filled_pages();
            builder = builder.character(&character.name, &character.save)?;
            for page in CHARACTER_PAGES {
                if !filled.contains(&page) {
                    builder = builder.grid(&character.grid(page), Vec::new())?;
                }
            }
        }
        Ok(builder.build())
    }

    pub fn project(&mut self, vault: &Vault) -> Result<(), WorldError> {
        let mut stashes: Vec<StashSave> =
            self.stashes.iter().map(|file| file.save.clone()).collect();
        for (file, save) in self.stashes.iter().zip(&mut stashes) {
            file.project(save, vault)?;
        }
        let mut characters: Vec<CharacterSave> = self
            .characters
            .iter()
            .map(|file| file.save.clone())
            .collect();
        for (file, save) in self.characters.iter().zip(&mut characters) {
            file.project(save, vault)?;
        }
        for (file, save) in self.stashes.iter_mut().zip(stashes) {
            file.save = save;
        }
        for (file, save) in self.characters.iter_mut().zip(characters) {
            file.save = save;
        }
        Ok(())
    }

    pub fn flush(&mut self, files: &dyn Files) -> Result<Vec<PathBuf>, WorldError> {
        let mut written = Vec::new();
        for stash in &mut self.stashes {
            let bytes = stash.save.write().map_err(|source| WorldError::Bitstream {
                path: stash.path.clone(),
                source,
            })?;
            if bytes != stash.bytes {
                files.write(&stash.path, &bytes)?;
                stash.bytes = bytes;
                written.push(stash.path.clone());
            }
        }
        for character in &mut self.characters {
            let bytes = character
                .save
                .write()
                .map_err(|source| WorldError::Bitstream {
                    path: character.path.clone(),
                    source,
                })?;
            if bytes != character.bytes {
                files.write(&character.path, &bytes)?;
                character.bytes = bytes;
                written.push(character.path.clone());
            }
        }
        Ok(written)
    }
}

impl StashFile {
    fn project(&self, save: &mut StashSave, vault: &Vault) -> Result<(), WorldError> {
        let name = &self.name;
        let path = &self.path;
        let count = save.tabs.len();
        for (index, tab) in save.tabs.iter_mut().enumerate() {
            let position = u8::try_from(index).map_err(|_| WorldError::TooManyTabs {
                path: path.clone(),
                tabs: count,
                max: u8::MAX,
            })?;
            let index = TabIndex::new(position);
            match &mut tab.body {
                TabBody::Chronicle(_) => {}
                TabBody::Normal(list) => {
                    let id = GridId::SharedTab {
                        stash: name.clone(),
                        tab: index,
                    };
                    let grid = vault
                        .grid(&id)
                        .ok_or_else(|| WorldError::UnknownContainer {
                            container: ContainerId::Grid(id.clone()),
                        })?;
                    let items = grid_items(grid, StorePage::Stash, &list.0)?;
                    *list = ItemList(items);
                }
                TabBody::AdvancedStash(list) => {
                    let id = PileId::new(name.clone(), index);
                    let pile = vault
                        .pile(&id)
                        .ok_or_else(|| WorldError::UnknownContainer {
                            container: ContainerId::Pile(id.clone()),
                        })?;
                    let items = pile_items(&ContainerId::Pile(id), pile, &list.0)?;
                    *list = ItemList(items);
                }
            }
        }
        Ok(())
    }
}

impl CharacterFile {
    fn grid(&self, page: CharacterPage) -> GridId {
        GridId::Character {
            character: self.name.clone(),
            page,
        }
    }

    fn filled_pages(&self) -> BTreeSet<CharacterPage> {
        self.save.items.0.iter().filter_map(stored_page).collect()
    }

    fn project(&self, save: &mut CharacterSave, vault: &Vault) -> Result<(), WorldError> {
        let mut placed: Vec<(CharacterPage, GridPosition, Item)> = Vec::new();
        for page in CHARACTER_PAGES {
            let id = self.grid(page);
            let grid = vault
                .grid(&id)
                .ok_or_else(|| WorldError::UnknownContainer {
                    container: ContainerId::Grid(id.clone()),
                })?;
            for slot in grid.slots() {
                placed.push((page, slot.origin(), slot.object().item().clone()));
            }
        }
        let held: Vec<(CharacterPage, Fingerprint)> = placed
            .iter()
            .map(|(page, _, item)| Ok((*page, Fingerprint::of(item)?)))
            .collect::<Result<_, ModelError>>()?;
        let original: Vec<Option<(CharacterPage, Fingerprint)>> = save
            .items
            .0
            .iter()
            .map(|item| match stored_page(item) {
                None => Ok(None),
                Some(page) => Ok(Some((page, Fingerprint::of(item)?))),
            })
            .collect::<Result<_, ModelError>>()?;

        let mut items = Vec::with_capacity(placed.len());
        for emit in arrange(&held, &original) {
            match emit {
                Emit::Original(index) => items.push(save.items.0[index].clone()),
                Emit::Held(index) => {
                    let (page, at, item) = &placed[index];
                    items.push(stored(item, *at, store_page(*page)));
                }
            }
        }
        save.items = ItemList(items);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Emit {
    Original(usize),
    Held(usize),
}

fn arrange<Key: Ord + Copy>(
    held: &[(Key, Fingerprint)],
    original: &[Option<(Key, Fingerprint)>],
) -> Vec<Emit> {
    let mut queues: BTreeMap<(Key, Fingerprint), VecDeque<usize>> = BTreeMap::new();
    for (index, key) in held.iter().enumerate() {
        queues.entry(*key).or_default().push_back(index);
    }
    let mut taken = vec![false; held.len()];
    let mut order = Vec::with_capacity(held.len().max(original.len()));
    for (index, entry) in original.iter().enumerate() {
        match entry {
            None => order.push(Emit::Original(index)),
            Some(key) => {
                if let Some(held) = queues.get_mut(key).and_then(VecDeque::pop_front) {
                    taken[held] = true;
                    order.push(Emit::Held(held));
                }
            }
        }
    }
    order.extend(
        taken
            .iter()
            .enumerate()
            .filter(|(_, kept)| !**kept)
            .map(|(index, _)| Emit::Held(index)),
    );
    order
}

fn grid_items(grid: &Grid, page: StorePage, original: &[Item]) -> Result<Vec<Item>, WorldError> {
    let slots = grid.slots();
    let held: Vec<((), Fingerprint)> = slots
        .iter()
        .map(|slot| ((), slot.object().fingerprint()))
        .collect();
    let original: Vec<Option<((), Fingerprint)>> = original
        .iter()
        .map(|item| Ok(Some(((), Fingerprint::of(item)?))))
        .collect::<Result<_, ModelError>>()?;
    Ok(arrange(&held, &original)
        .into_iter()
        .filter_map(|emit| match emit {
            Emit::Original(_) => None,
            Emit::Held(index) => Some(index),
        })
        .map(|index| {
            let slot = &slots[index];
            stored(slot.object().item(), slot.origin(), page)
        })
        .collect())
}

fn pile_items(
    container: &ContainerId,
    pile: &Pile,
    original: &[Item],
) -> Result<Vec<Item>, WorldError> {
    let entries = pile.entries();
    let held: Vec<((), Fingerprint)> = entries
        .iter()
        .map(|entry| ((), entry.object().fingerprint()))
        .collect();
    let original: Vec<Option<((), Fingerprint)>> = original
        .iter()
        .map(|item| Ok(Some(((), Fingerprint::of(item)?))))
        .collect::<Result<_, ModelError>>()?;
    arrange(&held, &original)
        .into_iter()
        .filter_map(|emit| match emit {
            Emit::Original(_) => None,
            Emit::Held(index) => Some(index),
        })
        .map(|index| {
            let entry = &entries[index];
            stacked(container, entry.object().item(), entry.count())
        })
        .collect()
}

fn stored(item: &Item, at: GridPosition, page: StorePage) -> Item {
    let mut placed = item.clone();
    placed.position = ItemPosition::Slot {
        mode: SlotMode::Stored,
        body: BodyLocation::None,
        cell: at.to_cell(),
        page,
    };
    placed
}

fn stacked(container: &ContainerId, item: &Item, count: PileCount) -> Result<Item, WorldError> {
    let stack = Bits::<8>::new(u32::from(count.get())).ok_or(WorldError::StackTooLarge {
        container: container.clone(),
        count: count.get(),
    })?;
    let mut piled = item.clone();
    let field = match &mut piled.body {
        ItemBody::Compact(body) => &mut body.advanced_stash_stack,
        ItemBody::Complete(body) => &mut body.advanced_stash_stack,
    };
    match field {
        Some(slot) => *slot = stack,
        None => {
            return Err(WorldError::StackAbsent {
                container: container.clone(),
                fingerprint: Fingerprint::of(item)?,
            });
        }
    }
    Ok(piled)
}

fn stored_page(item: &Item) -> Option<CharacterPage> {
    let ItemPosition::Slot {
        mode: SlotMode::Stored,
        page,
        ..
    } = item.position
    else {
        return None;
    };
    character_page(page)
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

const fn store_page(page: CharacterPage) -> StorePage {
    match page {
        CharacterPage::Inventory => StorePage::Inventory,
        CharacterPage::Stash => StorePage::Stash,
        CharacterPage::Cube => StorePage::Cube,
    }
}
