use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use app::sync::MIRROR_DIRECTORY;
use app::transaction::DEFAULT_TIMER;
use server::DEFAULT_PORT;

const SAVES_HELP: &str = "Directory holding the .d2s and .d2i files";
const CONTAINER_HELP: &str =
    "Keep one container only, as shared/<stash>/<tab> or char/<name>/<page>";

#[derive(Debug, Parser)]
#[command(
    name = "d2r-manager",
    version,
    about = "Diablo II Resurrected offline saves: backup, exploration and transactions"
)]
pub struct Options {
    #[command(subcommand)]
    pub task: Task,
}

#[derive(Debug, Subcommand)]
pub enum Task {
    #[command(about = "Mirror the game saves into the backup repository and commit the change")]
    Sync(SyncOptions),

    #[command(about = "List every item the saves hold, container by container")]
    Ls(LsOptions),

    #[command(about = "List the items a filter keeps")]
    Find(FindOptions),

    #[command(about = "Compare two save directories, container by container")]
    Diff(DiffOptions),

    #[command(about = "Print the JSON of a plan, without writing anything")]
    Plan(PlanOptions),

    #[command(about = "Apply a plan to the saves of a git repository")]
    Apply(ApplyOptions),

    #[command(about = "Serve the same tasks over HTTP, on the loopback of this machine only")]
    Serve(ServeOptions),
}

#[derive(Debug, Args)]
pub struct SyncOptions {
    #[arg(long, help = "Directory the game writes its saves in")]
    pub source: PathBuf,

    #[arg(long, help = "Backup repository, on main, with an origin remote")]
    pub repo: PathBuf,

    #[arg(long, help = "Lock file that keeps two runs from mirroring at once")]
    pub lock: PathBuf,
}

#[derive(Debug, Args)]
pub struct LsOptions {
    #[arg(long, help = SAVES_HELP)]
    pub saves: PathBuf,

    #[arg(long, help = CONTAINER_HELP)]
    pub container: Option<String>,
}

#[derive(Debug, Args)]
pub struct FindOptions {
    #[arg(long, help = SAVES_HELP)]
    pub saves: PathBuf,

    #[arg(long, help = CONTAINER_HELP)]
    pub container: Option<String>,

    #[arg(long, help = "Keep one item code only, such as jew or cm1")]
    pub code: Option<String>,

    #[arg(long, help = "Keep one quality only, such as unique or rare")]
    pub quality: Option<String>,

    #[arg(
        long,
        help = "Keep the fingerprints starting with these hexadecimal digits"
    )]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Args)]
pub struct DiffOptions {
    #[arg(long, help = "Directory read as the state before")]
    pub saves: PathBuf,

    #[arg(long, help = "Directory read as the state after")]
    pub against: PathBuf,
}

#[derive(Debug, Args)]
#[command(after_help = OPERATION_HELP)]
pub struct PlanOptions {
    #[arg(long, help = SAVES_HELP)]
    pub saves: PathBuf,

    #[arg(
        long = "operation",
        required = true,
        help = "One operation, repeated in the order the plan applies them"
    )]
    pub operations: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ApplyOptions {
    #[arg(
        long,
        help = "Git repository holding the saves and the provenance ledger"
    )]
    pub repo: PathBuf,

    #[arg(
        long,
        help = "Directory holding the .d2s and .d2i files, inside the repository"
    )]
    pub saves: Option<PathBuf>,

    #[arg(long, help = "JSON file the plan command printed")]
    pub plan: PathBuf,

    #[arg(
        long,
        default_value = DEFAULT_TIMER,
        help = "Systemd user timer paused while the plan is applied"
    )]
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

#[derive(Debug, Args)]
pub struct ServeOptions {
    #[arg(
        long,
        default_value_t = DEFAULT_PORT,
        help = "Port the API listens on, 127.0.0.1 only; 0 takes any free port"
    )]
    pub port: u16,
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
