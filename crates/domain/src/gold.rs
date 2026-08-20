use std::fmt;

use crate::error::ValueError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gold(u32);

impl Gold {
    pub const CAP: u32 = 2_500_000;
    pub const ZERO: Self = Self(0);

    pub const fn new(amount: u32) -> Result<Self, ValueError> {
        if amount > Self::CAP {
            return Err(ValueError::GoldOverCap {
                amount,
                cap: Self::CAP,
            });
        }
        Ok(Self(amount))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    pub const fn try_add(self, other: Self) -> Result<Self, ValueError> {
        Self::new(self.0 + other.0)
    }
}

impl fmt::Display for Gold {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
