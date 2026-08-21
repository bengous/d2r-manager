pub mod options;
pub mod render;

use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Result, anyhow};

use app::inspect::{FingerprintPrefix, Needle, Quality, compare, list};
use app::selector::ContainerSelector;
use app::sync::{Backup, BackupPaths};
use app::transaction::{PlanDocument, Transaction, TransactionPaths, describe, resolve};
use app::value::UnitName;
use app::world::SaveTree;
use domain::vault::Vault;
use format::code::ItemCode;

use infra::files::{FileLock, RealFiles};
use infra::git::GitCommands;
use infra::system::{Rsync, SystemClock, SystemProcesses, SystemdTimers};
use options::{
    ApplyOptions, DiffOptions, FindOptions, LsOptions, Options, PlanOptions, SyncOptions, Task,
};

pub fn run(options: &Options, out: &mut dyn Write) -> Result<()> {
    match &options.task {
        Task::Sync(options) => sync(options, out),
        Task::Ls(options) => ls(options, out),
        Task::Find(options) => find(options, out),
        Task::Diff(options) => diff(options, out),
        Task::Plan(options) => plan(options, out),
        Task::Apply(options) => apply(options, out),
    }
}

fn sync(options: &SyncOptions, out: &mut dyn Write) -> Result<()> {
    let files = RealFiles;
    let locks = FileLock::default();
    let processes = SystemProcesses;
    let mirrors = Rsync;
    let repository = GitCommands;
    let clock = SystemClock;
    let backup = Backup {
        files: &files,
        locks: &locks,
        processes: &processes,
        mirrors: &mirrors,
        repository: &repository,
        clock: &clock,
    };
    let outcome = backup.run(&BackupPaths {
        source: options.source.clone(),
        repo: options.repo.clone(),
        lock: options.lock.clone(),
    })?;
    writeln!(out, "{}", render::sync(&outcome))?;
    Ok(())
}

fn ls(options: &LsOptions, out: &mut dyn Write) -> Result<()> {
    let vault = world(&options.saves)?;
    let needle = Needle {
        container: container(options.container.as_deref())?,
        ..Needle::default()
    };
    write!(out, "{}", render::rows(&list(&vault, &needle)))?;
    Ok(())
}

fn find(options: &FindOptions, out: &mut dyn Write) -> Result<()> {
    let vault = world(&options.saves)?;
    let needle = Needle {
        container: container(options.container.as_deref())?,
        code: options
            .code
            .as_deref()
            .map(ItemCode::from_text)
            .transpose()?,
        quality: options.quality.as_deref().map(Quality::parse).transpose()?,
        fingerprint: options
            .fingerprint
            .as_deref()
            .map(FingerprintPrefix::new)
            .transpose()?,
    };
    write!(out, "{}", render::rows(&list(&vault, &needle)))?;
    Ok(())
}

fn diff(options: &DiffOptions, out: &mut dyn Write) -> Result<()> {
    let before = world(&options.saves)?;
    let after = world(&options.against)?;
    write!(out, "{}", render::differences(&compare(&before, &after)))?;
    Ok(())
}

fn plan(options: &PlanOptions, out: &mut dyn Write) -> Result<()> {
    let document = PlanDocument {
        operations: options
            .operations
            .iter()
            .map(|text| app::shorthand::parse(text))
            .collect::<Result<_, _>>()?,
    };
    let vault = world(&options.saves)?;
    let plan = resolve(&document, &vault)?;
    let normalized = describe(&plan);
    plan.validate(vault)?;
    writeln!(out, "{}", serde_json::to_string_pretty(&normalized)?)?;
    Ok(())
}

fn apply(options: &ApplyOptions, out: &mut dyn Write) -> Result<()> {
    let plan = options.plan.display();
    let text = std::fs::read_to_string(&options.plan)
        .map_err(|source| anyhow!("{plan}: read: {source}"))?;
    let document: PlanDocument = serde_json::from_str(&text)
        .map_err(|source| anyhow!("{plan}: the plan does not parse: {source}"))?;
    let files = RealFiles;
    let processes = SystemProcesses;
    let repository = GitCommands;
    let timers = SystemdTimers;
    let clock = SystemClock;
    let transaction = Transaction {
        files: &files,
        processes: &processes,
        repository: &repository,
        timers: &timers,
        clock: &clock,
    };
    let applied = transaction.apply(
        &TransactionPaths {
            repo: options.repo.clone(),
            saves: options.saves(),
            timer: UnitName::new(&options.timer)?,
        },
        &document,
    )?;
    writeln!(out, "{}", render::applied(&applied))?;
    Ok(())
}

fn world(directory: &Path) -> Result<Vault> {
    let files = RealFiles;
    Ok(SaveTree::read(&files, directory)?.vault()?)
}

fn container(text: Option<&str>) -> Result<Option<ContainerSelector>> {
    Ok(text.map(ContainerSelector::from_str).transpose()?)
}
