use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Serialize;
use serde::de::DeserializeOwned;

use app::inspect::{FingerprintPrefix, Needle, Quality, compare, list};
use app::ports::Files;
use app::selector::ContainerSelector;
use app::sync::{Backup, BackupPaths, MIRROR_DIRECTORY};
use app::transaction::{DEFAULT_TIMER, Transaction, TransactionPaths, describe, resolve};
use app::value::UnitName;
use app::world::SaveTree;
use domain::vault::Vault;
use format::code::ItemCode;

use crate::Services;
use crate::failure::{Failure, Kind};
use crate::payload::{
    ApplyBody, ApplyRequest, DiffBody, DifferenceBody, ItemBody, ItemsBody, PlanBody, PlanRequest,
    SyncBody, SyncRequest,
};
use crate::query::Query;
use crate::route::Target;

const ITEM_PARAMETERS: [&str; 5] = ["saves", "container", "code", "quality", "fingerprint"];
const DIFF_PARAMETERS: [&str; 2] = ["saves", "against"];

pub(crate) fn answer(
    services: Services<'_>,
    target: Target,
    query: &str,
    body: &[u8],
) -> Result<String, Failure> {
    match target {
        Target::Sync => write(&sync(services, &parse(query, body)?)?),
        Target::Items => write(&items(services, query)?),
        Target::Diff => write(&diff(services, query)?),
        Target::Plan => write(&plan(services, &parse(query, body)?)?),
        Target::Apply => write(&apply(services, &parse(query, body)?)?),
    }
}

fn sync(services: Services<'_>, request: &SyncRequest) -> Result<SyncBody, Failure> {
    let backup = Backup {
        files: services.files,
        locks: services.locks,
        processes: services.processes,
        mirrors: services.mirrors,
        repository: services.repository,
        clock: services.clock,
    };
    let outcome = backup.run(&BackupPaths {
        source: request.source.clone(),
        repo: request.repo.clone(),
        lock: request.lock.clone(),
    })?;
    Ok(SyncBody::of(&outcome))
}

fn items(services: Services<'_>, query: &str) -> Result<ItemsBody, Failure> {
    let query = Query::parse(query, &ITEM_PARAMETERS)?;
    let vault = world(services.files, Path::new(query.required("saves")?))?;
    let needle = Needle {
        container: query
            .optional("container")
            .map(ContainerSelector::from_str)
            .transpose()?,
        code: query
            .optional("code")
            .map(ItemCode::from_text)
            .transpose()?,
        quality: query.optional("quality").map(Quality::parse).transpose()?,
        fingerprint: query
            .optional("fingerprint")
            .map(FingerprintPrefix::new)
            .transpose()?,
    };
    Ok(ItemsBody {
        items: list(&vault, &needle).iter().map(ItemBody::of).collect(),
    })
}

fn diff(services: Services<'_>, query: &str) -> Result<DiffBody, Failure> {
    let query = Query::parse(query, &DIFF_PARAMETERS)?;
    let before = world(services.files, Path::new(query.required("saves")?))?;
    let after = world(services.files, Path::new(query.required("against")?))?;
    Ok(DiffBody {
        differences: compare(&before, &after)
            .iter()
            .map(DifferenceBody::of)
            .collect(),
    })
}

fn plan(services: Services<'_>, request: &PlanRequest) -> Result<PlanBody, Failure> {
    let vault = world(services.files, &request.saves)?;
    let plan = resolve(&request.plan, &vault)?;
    let normalized = describe(&plan);
    plan.validate(vault)?;
    Ok(PlanBody { plan: normalized })
}

fn apply(services: Services<'_>, request: &ApplyRequest) -> Result<ApplyBody, Failure> {
    let transaction = Transaction {
        files: services.files,
        processes: services.processes,
        repository: services.repository,
        timers: services.timers,
        clock: services.clock,
    };
    let applied = transaction.apply(
        &TransactionPaths {
            repo: request.repo.clone(),
            saves: saves(request),
            timer: UnitName::new(request.timer.as_deref().unwrap_or(DEFAULT_TIMER))?,
        },
        &request.plan,
    )?;
    Ok(ApplyBody::of(&applied))
}

fn saves(request: &ApplyRequest) -> PathBuf {
    request
        .saves
        .clone()
        .unwrap_or_else(|| request.repo.join(MIRROR_DIRECTORY))
}

fn world(files: &dyn Files, directory: &Path) -> Result<Vault, Failure> {
    Ok(SaveTree::read(files, directory)?.vault()?)
}

fn parse<Body: DeserializeOwned>(query: &str, body: &[u8]) -> Result<Body, Failure> {
    if !query.is_empty() {
        return Err(Failure::new(
            Kind::BadRequest,
            format!("this route reads its request from the body alone, not from {query:?}"),
        ));
    }
    serde_json::from_slice(body).map_err(|error| {
        Failure::new(
            Kind::BadRequest,
            format!("the request body is no such document: {error}"),
        )
    })
}

fn write<Body: Serialize>(body: &Body) -> Result<String, Failure> {
    serde_json::to_string(body).map_err(|error| {
        Failure::new(
            Kind::Internal,
            format!("the answer does not write: {error}"),
        )
    })
}
