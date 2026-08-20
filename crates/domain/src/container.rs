use std::fmt;

use crate::error::ValueError;
use crate::grid::GridGeometry;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StashName(Box<str>);

impl StashName {
    pub fn new(name: &str) -> Result<Self, ValueError> {
        if name.is_empty() {
            return Err(ValueError::EmptyName);
        }
        Ok(Self(name.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StashName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharacterName(Box<str>);

impl CharacterName {
    pub fn new(name: &str) -> Result<Self, ValueError> {
        if name.is_empty() {
            return Err(ValueError::EmptyName);
        }
        Ok(Self(name.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CharacterName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabIndex(u8);

impl TabIndex {
    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for TabIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CharacterPage {
    Inventory,
    Stash,
    Cube,
}

impl CharacterPage {
    #[must_use]
    pub const fn geometry(self) -> GridGeometry {
        match self {
            Self::Inventory => GridGeometry::INVENTORY,
            Self::Stash => GridGeometry::CHARACTER_STASH,
            Self::Cube => GridGeometry::CUBE,
        }
    }
}

impl fmt::Display for CharacterPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inventory => "inventory",
            Self::Stash => "stash",
            Self::Cube => "cube",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GridId {
    SharedTab {
        stash: StashName,
        tab: TabIndex,
    },
    Character {
        character: CharacterName,
        page: CharacterPage,
    },
}

impl GridId {
    #[must_use]
    pub const fn geometry(&self) -> GridGeometry {
        match self {
            Self::SharedTab { .. } => GridGeometry::SHARED_TAB,
            Self::Character { page, .. } => page.geometry(),
        }
    }

    #[must_use]
    pub const fn character(&self) -> Option<&CharacterName> {
        match self {
            Self::SharedTab { .. } => None,
            Self::Character { character, .. } => Some(character),
        }
    }
}

impl fmt::Display for GridId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SharedTab { stash, tab } => write!(formatter, "stash {stash} tab {tab}"),
            Self::Character { character, page } => write!(formatter, "{character} {page}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileId {
    stash: StashName,
    tab: TabIndex,
}

impl PileId {
    #[must_use]
    pub const fn new(stash: StashName, tab: TabIndex) -> Self {
        Self { stash, tab }
    }

    #[must_use]
    pub const fn stash(&self) -> &StashName {
        &self.stash
    }

    #[must_use]
    pub const fn tab(&self) -> TabIndex {
        self.tab
    }
}

impl fmt::Display for PileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "stash {} tab {} pile", self.stash, self.tab)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContainerId {
    Grid(GridId),
    Pile(PileId),
}

impl ContainerId {
    #[must_use]
    pub const fn character(&self) -> Option<&CharacterName> {
        match self {
            Self::Grid(grid) => grid.character(),
            Self::Pile(_) => None,
        }
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grid(grid) => grid.fmt(formatter),
            Self::Pile(pile) => pile.fmt(formatter),
        }
    }
}

impl From<GridId> for ContainerId {
    fn from(id: GridId) -> Self {
        Self::Grid(id)
    }
}

impl From<PileId> for ContainerId {
    fn from(id: PileId) -> Self {
        Self::Pile(id)
    }
}
