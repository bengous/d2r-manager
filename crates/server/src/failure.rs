use std::fmt;
use std::io::ErrorKind;

use app::inspect::NeedleError;
use app::ports::FileError;
use app::selector::SelectorError;
use app::sync::SyncError;
use app::transaction::{ResolveError, TransactionError};
use app::value::ValueError;
use app::world::WorldError;
use domain::error::{PlanError, VaultError};
use format::error::FormatError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    BadRequest,
    ForbiddenOrigin,
    NoSuchRoute,
    NotFound,
    MethodNotAllowed,
    PayloadTooLarge,
    GameRunning,
    PathRefused,
    RepositoryState,
    RuleRefused,
    ProjectionRefused,
    UnsupportedSave,
    Internal,
}

impl Kind {
    pub const ALL: [Self; 13] = [
        Self::BadRequest,
        Self::ForbiddenOrigin,
        Self::NoSuchRoute,
        Self::NotFound,
        Self::MethodNotAllowed,
        Self::PayloadTooLarge,
        Self::GameRunning,
        Self::PathRefused,
        Self::RepositoryState,
        Self::RuleRefused,
        Self::ProjectionRefused,
        Self::UnsupportedSave,
        Self::Internal,
    ];

    #[must_use]
    pub const fn status(self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::ForbiddenOrigin => 403,
            Self::NoSuchRoute | Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::GameRunning | Self::PathRefused | Self::RepositoryState => 409,
            Self::PayloadTooLarge => 413,
            Self::RuleRefused | Self::ProjectionRefused | Self::UnsupportedSave => 422,
            Self::Internal => 500,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadRequest => "bad_request",
            Self::ForbiddenOrigin => "forbidden_origin",
            Self::NoSuchRoute => "no_such_route",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::PayloadTooLarge => "payload_too_large",
            Self::GameRunning => "game_running",
            Self::PathRefused => "path_refused",
            Self::RepositoryState => "repository_state",
            Self::RuleRefused => "rule_refused",
            Self::ProjectionRefused => "projection_refused",
            Self::UnsupportedSave => "unsupported_save",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    kind: Kind,
    message: String,
}

impl Failure {
    #[must_use]
    pub fn new(kind: Kind, message: String) -> Self {
        Self { kind, message }
    }

    fn of(kind: Kind, error: &dyn fmt::Display) -> Self {
        Self::new(kind, error.to_string())
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub const fn status(&self) -> u16 {
        self.kind.status()
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn body(&self) -> String {
        let detail = Detail {
            kind: self.kind.as_str(),
            message: &self.message,
        };
        serde_json::to_string(&Envelope { error: detail })
            .expect("an envelope of two strings always writes")
    }
}

#[derive(Debug, serde::Serialize)]
struct Envelope<'a> {
    error: Detail<'a>,
}

#[derive(Debug, serde::Serialize)]
struct Detail<'a> {
    kind: &'a str,
    message: &'a str,
}

impl From<ValueError> for Failure {
    fn from(error: ValueError) -> Self {
        Self::of(Kind::BadRequest, &error)
    }
}

impl From<SelectorError> for Failure {
    fn from(error: SelectorError) -> Self {
        Self::of(Kind::BadRequest, &error)
    }
}

impl From<NeedleError> for Failure {
    fn from(error: NeedleError) -> Self {
        Self::of(Kind::BadRequest, &error)
    }
}

impl From<FormatError> for Failure {
    fn from(error: FormatError) -> Self {
        Self::of(Kind::BadRequest, &error)
    }
}

impl From<PlanError> for Failure {
    fn from(error: PlanError) -> Self {
        Self::of(Kind::RuleRefused, &error)
    }
}

impl From<WorldError> for Failure {
    fn from(error: WorldError) -> Self {
        Self::of(world(&error), &error)
    }
}

impl From<ResolveError> for Failure {
    fn from(error: ResolveError) -> Self {
        Self::of(resolve(&error), &error)
    }
}

impl From<SyncError> for Failure {
    fn from(error: SyncError) -> Self {
        let kind = match &error {
            SyncError::MissingCommand { .. }
            | SyncError::Command(_)
            | SyncError::Value(_)
            | SyncError::LockWithoutDirectory { .. } => Kind::Internal,
            SyncError::UnsafeSource { .. }
            | SyncError::UnsafeRepository { .. }
            | SyncError::UnexpectedRoot { .. }
            | SyncError::UnsafeMirror { .. }
            | SyncError::UnexpectedMirror { .. }
            | SyncError::LockRootMissing { .. } => Kind::PathRefused,
            SyncError::DetachedHead
            | SyncError::WrongBranch { .. }
            | SyncError::MissingRemote { .. } => Kind::RepositoryState,
            SyncError::File(source) => file(source),
        };
        Self::of(kind, &error)
    }
}

impl From<TransactionError> for Failure {
    fn from(error: TransactionError) -> Self {
        Self::of(transaction(&error), &error)
    }
}

fn transaction(error: &TransactionError) -> Kind {
    match error {
        TransactionError::GameRunning => Kind::GameRunning,
        TransactionError::UnexpectedRoot { .. }
        | TransactionError::SavesOutsideRepository { .. } => Kind::PathRefused,
        TransactionError::LedgerWithoutDirectory { .. }
        | TransactionError::ReadBack(_)
        | TransactionError::Command(_)
        | TransactionError::Json(_)
        | TransactionError::Clock(_)
        | TransactionError::TimerLeftPaused { .. } => Kind::Internal,
        TransactionError::Resolve(source) => resolve(source),
        TransactionError::Refused(_) => Kind::RuleRefused,
        TransactionError::World(source) => world(source),
        TransactionError::File(source) => file(source),
        TransactionError::Compound { first, .. } => transaction(first),
    }
}

fn world(error: &WorldError) -> Kind {
    match error {
        WorldError::File(source) => file(source),
        WorldError::NotADirectory { .. } => Kind::NotFound,
        WorldError::Unnamed { .. }
        | WorldError::Name { .. }
        | WorldError::Bitstream { .. }
        | WorldError::Model(_)
        | WorldError::TooManyTabs { .. } => Kind::UnsupportedSave,
        WorldError::Vault(source) => vault(source),
        WorldError::UnknownContainer { .. }
        | WorldError::StackAbsent { .. }
        | WorldError::StackTooLarge { .. } => Kind::ProjectionRefused,
    }
}

fn vault(error: &VaultError) -> Kind {
    match error {
        VaultError::Placement(_) => Kind::RuleRefused,
        VaultError::Object { .. }
        | VaultError::Gold { .. }
        | VaultError::Pile { .. }
        | VaultError::NotStored { .. }
        | VaultError::ContainerDeclaredTwice { .. }
        | VaultError::TooManyTabs { .. } => Kind::UnsupportedSave,
    }
}

fn resolve(error: &ResolveError) -> Kind {
    match error {
        ResolveError::Selector { .. }
        | ResolveError::Fingerprint { .. }
        | ResolveError::Units { .. }
        | ResolveError::Cell { .. } => Kind::BadRequest,
        ResolveError::UnknownContainer { .. }
        | ResolveError::GridSourceUnits { .. }
        | ResolveError::PileDestinationCell { .. } => Kind::RuleRefused,
    }
}

fn file(error: &FileError) -> Kind {
    if error.source.kind() == ErrorKind::NotFound {
        Kind::NotFound
    } else {
        Kind::Internal
    }
}
