use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use app::sync::MIRROR_DIRECTORY;

pub const DEFAULT_TIMER: &str = "d2r-backup.timer";

#[derive(Debug, Parser)]
#[command(
    name = "d2r-manager",
    version,
    about = "Diablo II Resurrected offline saves: backup, exploration and transactions",
    long_about = None
)]
pub struct Options {
    #[command(subcommand)]
    pub task: Task,
}

#[derive(Debug, Subcommand)]
pub enum Task {
    /// Mirror the game saves into the backup repository and commit the change
    Sync(SyncOptions),
    /// List every item the saves hold, container by container
    Ls(LsOptions),
    /// List the items a filter keeps
    Find(FindOptions),
    /// Compare two save directories, container by container
    Diff(DiffOptions),
    /// Print the JSON of a plan, without writing anything
    Plan(PlanOptions),
    /// Apply a plan to the saves of a git repository
    Apply(ApplyOptions),
}

#[derive(Debug, Args)]
pub struct SyncOptions {
    /// Directory the game writes its saves in
    #[arg(long)]
    pub source: PathBuf,
    /// Backup repository, on main, with an origin remote
    #[arg(long)]
    pub repo: PathBuf,
    /// Lock file that keeps two runs from mirroring at once
    #[arg(long)]
    pub lock: PathBuf,
}

#[derive(Debug, Args)]
pub struct LsOptions {
    /// Directory holding the .d2s and .d2i files
    #[arg(long)]
    pub saves: PathBuf,
    /// Keep one container only, as shared/<stash>/<tab> or char/<name>/<page>
    #[arg(long)]
    pub container: Option<String>,
}

#[derive(Debug, Args)]
pub struct FindOptions {
    /// Directory holding the .d2s and .d2i files
    #[arg(long)]
    pub saves: PathBuf,
    /// Keep one container only, as shared/<stash>/<tab> or char/<name>/<page>
    #[arg(long)]
    pub container: Option<String>,
    /// Keep one item code only, such as jew or cm1
    #[arg(long)]
    pub code: Option<String>,
    /// Keep one quality only, such as unique or rare
    #[arg(long)]
    pub quality: Option<String>,
    /// Keep the fingerprints starting with these hexadecimal digits
    #[arg(long)]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Args)]
pub struct DiffOptions {
    /// Directory read as the state before
    #[arg(long)]
    pub saves: PathBuf,
    /// Directory read as the state after
    #[arg(long)]
    pub against: PathBuf,
}

#[derive(Debug, Args)]
#[command(after_help = OPERATION_HELP)]
pub struct PlanOptions {
    /// Directory holding the .d2s and .d2i files
    #[arg(long)]
    pub saves: PathBuf,
    /// One operation, repeated in the order the plan applies them
    #[arg(long = "operation", required = true)]
    pub operations: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ApplyOptions {
    /// Git repository holding the saves and the provenance ledger
    #[arg(long)]
    pub repo: PathBuf,
    /// Directory holding the .d2s and .d2i files, inside the repository
    #[arg(long)]
    pub saves: Option<PathBuf>,
    /// JSON file the plan command printed
    #[arg(long)]
    pub plan: PathBuf,
    /// Systemd user timer paused while the plan is applied
    #[arg(long, default_value = DEFAULT_TIMER)]
    pub timer: String,
}

impl ApplyOptions {
    #[must_use]
    pub fn saves(&self) -> PathBuf {
        self.saves
            .clone()
            .unwrap_or_else(|| self.repo.join(MIRROR_DIRECTORY))
    }
}

const OPERATION_HELP: &str = concat!(
    "An operation reads as <verb>:<source>:<destination>\n",
    "  verb         move or duplicate\n",
    "  source       <container>#<fingerprint> with *<units> for an advanced stash tab\n",
    "  destination  <container> with @<column>,<row> for a named cell\n",
    "  container    shared/<stash>/<tab> or char/<name>/<inventory|stash|cube>\n",
    "\n",
    "Examples:\n",
    "  move:shared/linux/3#0f1e2d3c4b5a69788796a5b4c3d2e1f0:shared/linux/4@0,2\n",
    "  move:shared/linux/5#0f1e2d3c4b5a69788796a5b4c3d2e1f0*20:shared/linux/5\n",
    "  duplicate:char/Gous/stash#0f1e2d3c4b5a69788796a5b4c3d2e1f0:char/Elektra/cube\n",
);
