use std::fmt;

use thiserror::Error;

const COMMIT_DIGITS: usize = 40;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValueError {
    #[error("a commit reads as {COMMIT_DIGITS} lowercase hexadecimal digits, not {found:?}")]
    Commit { found: String },

    #[error("a branch name holds no blank and is never empty, unlike {found:?}")]
    Branch { found: String },

    #[error("a timestamp holds no control character and is never empty, unlike {found:?}")]
    Timestamp { found: String },

    #[error("a systemd unit name holds no blank and is never empty, unlike {found:?}")]
    Unit { found: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId(Box<str>);

impl CommitId {
    pub fn new(text: &str) -> Result<Self, ValueError> {
        let digits = text.trim();
        if digits.len() != COMMIT_DIGITS
            || !digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ValueError::Commit {
                found: text.to_owned(),
            });
        }
        Ok(Self(digits.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Branch(Box<str>);

impl Branch {
    pub fn new(text: &str) -> Result<Self, ValueError> {
        let name = text.trim_end_matches('\n');
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(ValueError::Branch {
                found: text.to_owned(),
            });
        }
        Ok(Self(name.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(Box<str>);

impl Timestamp {
    pub fn new(text: &str) -> Result<Self, ValueError> {
        if text.is_empty() || text.chars().any(char::is_control) {
            return Err(ValueError::Timestamp {
                found: text.to_owned(),
            });
        }
        Ok(Self(text.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitName(Box<str>);

impl UnitName {
    pub fn new(text: &str) -> Result<Self, ValueError> {
        if text.is_empty() || text.chars().any(char::is_whitespace) {
            return Err(ValueError::Unit {
                found: text.to_owned(),
            });
        }
        Ok(Self(text.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnitName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
