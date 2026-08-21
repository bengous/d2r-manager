use thiserror::Error;

use crate::transaction::{
    CellDocument, DestinationDocument, OperationDocument, SourceDocument, TransferDocument,
};

pub const MOVE: &str = "move";
pub const DUPLICATE: &str = "duplicate";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShorthandError {
    #[error("an operation reads as <verb>:<source>:<destination>, not {found:?}")]
    Shape { found: String },

    #[error("{found:?} names no transfer: {MOVE} and {DUPLICATE} do")]
    Transfer { found: String },

    #[error("a source reads as <container>#<fingerprint> with an optional *<units>, not {found:?}")]
    Source { found: String },

    #[error("a destination reads as <container> with an optional @<column>,<row>, not {found:?}")]
    Destination { found: String },

    #[error("a unit count reads as a number from 1 to 99, not {found:?}")]
    Units { found: String },

    #[error("a cell reads as <column>,<row>, not {found:?}")]
    Cell { found: String },
}

pub fn parse(text: &str) -> Result<OperationDocument, ShorthandError> {
    let mut parts = text.splitn(3, ':');
    let (Some(verb), Some(from), Some(to)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(ShorthandError::Shape {
            found: text.to_owned(),
        });
    };
    Ok(OperationDocument {
        transfer: transfer(verb)?,
        from: source(from)?,
        to: destination(to)?,
    })
}

fn transfer(text: &str) -> Result<TransferDocument, ShorthandError> {
    match text {
        MOVE => Ok(TransferDocument::Move),
        DUPLICATE => Ok(TransferDocument::Duplicate),
        _ => Err(ShorthandError::Transfer {
            found: text.to_owned(),
        }),
    }
}

fn source(text: &str) -> Result<SourceDocument, ShorthandError> {
    let (container, rest) = text.split_once('#').ok_or_else(|| ShorthandError::Source {
        found: text.to_owned(),
    })?;
    let (fingerprint, units) = match rest.split_once('*') {
        None => (rest, None),
        Some((fingerprint, count)) => (
            fingerprint,
            Some(count.parse().map_err(|_| ShorthandError::Units {
                found: count.to_owned(),
            })?),
        ),
    };
    if container.is_empty() || fingerprint.is_empty() {
        return Err(ShorthandError::Source {
            found: text.to_owned(),
        });
    }
    Ok(SourceDocument {
        container: container.to_owned(),
        fingerprint: fingerprint.to_owned(),
        units,
    })
}

fn destination(text: &str) -> Result<DestinationDocument, ShorthandError> {
    let (container, cell) = match text.split_once('@') {
        None => (text, None),
        Some((container, cell)) => (container, Some(position(cell)?)),
    };
    if container.is_empty() {
        return Err(ShorthandError::Destination {
            found: text.to_owned(),
        });
    }
    Ok(DestinationDocument {
        container: container.to_owned(),
        cell,
    })
}

fn position(text: &str) -> Result<CellDocument, ShorthandError> {
    let unreadable = || ShorthandError::Cell {
        found: text.to_owned(),
    };
    let (column, row) = text.split_once(',').ok_or_else(unreadable)?;
    Ok(CellDocument {
        column: column.parse().map_err(|_| unreadable())?,
        row: row.parse().map_err(|_| unreadable())?,
    })
}
