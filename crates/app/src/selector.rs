use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use domain::container::{
    CharacterName, CharacterPage, ContainerId, GridId, PileId, StashName, TabIndex,
};
use domain::error::ValueError;

pub const SHARED_PREFIX: &str = "shared";
pub const CHARACTER_PREFIX: &str = "char";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SelectorError {
    #[error(
        "a container reads as {SHARED_PREFIX}/<stash>/<tab> or {CHARACTER_PREFIX}/<character>/<page>, not {found:?}"
    )]
    Shape { found: String },

    #[error("{found:?} names no container family: {SHARED_PREFIX} and {CHARACTER_PREFIX} do")]
    Family { found: String },

    #[error("{found:?} names no character page: inventory, stash and cube do")]
    Page { found: String },

    #[error("a stash tab reads as a number from 0 to {max}, not {found:?}", max = u8::MAX)]
    Tab { found: String },

    #[error("container name: {0}")]
    Name(#[from] ValueError),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContainerSelector {
    SharedTab {
        stash: StashName,
        tab: TabIndex,
    },
    CharacterPage {
        character: CharacterName,
        page: CharacterPage,
    },
}

impl ContainerSelector {
    #[must_use]
    pub fn grid(&self) -> GridId {
        match self {
            Self::SharedTab { stash, tab } => GridId::SharedTab {
                stash: stash.clone(),
                tab: *tab,
            },
            Self::CharacterPage { character, page } => GridId::Character {
                character: character.clone(),
                page: *page,
            },
        }
    }

    #[must_use]
    pub fn pile(&self) -> Option<PileId> {
        match self {
            Self::SharedTab { stash, tab } => Some(PileId::new(stash.clone(), *tab)),
            Self::CharacterPage { .. } => None,
        }
    }

    #[must_use]
    pub fn of(container: &ContainerId) -> Self {
        match container {
            ContainerId::Grid(GridId::SharedTab { stash, tab }) => Self::SharedTab {
                stash: stash.clone(),
                tab: *tab,
            },
            ContainerId::Grid(GridId::Character { character, page }) => Self::CharacterPage {
                character: character.clone(),
                page: *page,
            },
            ContainerId::Pile(pile) => Self::SharedTab {
                stash: pile.stash().clone(),
                tab: pile.tab(),
            },
        }
    }
}

impl FromStr for ContainerSelector {
    type Err = SelectorError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut parts = text.split('/');
        let (Some(family), Some(name), Some(leaf), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(SelectorError::Shape {
                found: text.to_owned(),
            });
        };
        match family {
            SHARED_PREFIX => Ok(Self::SharedTab {
                stash: StashName::new(name)?,
                tab: TabIndex::new(leaf.parse().map_err(|_| SelectorError::Tab {
                    found: leaf.to_owned(),
                })?),
            }),
            CHARACTER_PREFIX => Ok(Self::CharacterPage {
                character: CharacterName::new(name)?,
                page: page(leaf)?,
            }),
            _ => Err(SelectorError::Family {
                found: family.to_owned(),
            }),
        }
    }
}

impl fmt::Display for ContainerSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SharedTab { stash, tab } => {
                write!(formatter, "{SHARED_PREFIX}/{stash}/{tab}")
            }
            Self::CharacterPage { character, page } => {
                write!(formatter, "{CHARACTER_PREFIX}/{character}/{page}")
            }
        }
    }
}

fn page(text: &str) -> Result<CharacterPage, SelectorError> {
    match text {
        "inventory" => Ok(CharacterPage::Inventory),
        "stash" => Ok(CharacterPage::Stash),
        "cube" => Ok(CharacterPage::Cube),
        _ => Err(SelectorError::Page {
            found: text.to_owned(),
        }),
    }
}
