mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use app::inspect::Quality;
use app::ledger::LedgerEntry;
use app::sync::MIRROR_DIRECTORY;
use app::transaction::{
    CellDocument, DEFAULT_TIMER, DestinationDocument, OperationDocument, PlanDocument,
    SourceDocument, TransferDocument,
};
use domain::pile::{PileCount, PileUnits};
use server::failure::{Failure, Kind};
use server::payload::{
    ApplyBody, DiffBody, DifferenceBody, ItemBody, ItemsBody, PlacementBody, PlanBody,
    RealizedBody, RecordBody, SyncBody,
};
use server::route::{Method, ROUTES};

use support::{ABSENT_TIMER, AT, Answer, Call, Rig, copy_fixture, encoded, git, repository, seed};

const CONTRACT: &str = include_str!("../../../docs/openapi.json");

const METHODS: [&str; 7] = ["get", "put", "post", "delete", "options", "head", "patch"];

const KEYWORDS: [&str; 13] = [
    "$ref",
    "additionalProperties",
    "const",
    "description",
    "enum",
    "items",
    "maximum",
    "minimum",
    "oneOf",
    "properties",
    "required",
    "type",
    "x-default",
];

#[derive(Debug)]
struct Contract {
    document: Value,
}

impl Contract {
    fn read() -> Self {
        Self {
            document: serde_json::from_str(CONTRACT).expect("the contract reads as JSON"),
        }
    }

    fn at(&self, keys: &[&str]) -> &Value {
        let mut held = &self.document;
        for key in keys {
            held = held
                .get(key)
                .unwrap_or_else(|| panic!("the contract declares no {}", keys.join("/")));
        }
        held
    }

    fn resolve<'a>(&'a self, schema: &'a Value) -> &'a Value {
        match schema.get("$ref").and_then(Value::as_str) {
            None => schema,
            Some(pointer) => {
                let keys: Vec<&str> = pointer.trim_start_matches("#/").split('/').collect();
                self.at(&keys)
            }
        }
    }

    fn check(&self, schema: &Value, value: &Value, at: &str) -> Result<(), String> {
        let schema = self.resolve(schema);
        if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
            let held = branches
                .iter()
                .filter(|branch| self.check(branch, value, at).is_ok())
                .count();
            return if held == 1 {
                Ok(())
            } else {
                Err(format!("{at}: {held} branches of oneOf hold {value}"))
            };
        }
        if let Some(constant) = schema.get("const")
            && constant != value
        {
            return Err(format!("{at}: {value} is not {constant}"));
        }
        if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
            && !allowed.contains(value)
        {
            return Err(format!("{at}: {value} is outside {allowed:?}"));
        }
        let named = schema.get("const").is_some() || schema.get("enum").is_some();
        match schema.get("type").and_then(Value::as_str) {
            None if named => Ok(()),
            None => Err(format!("{at}: the contract declares no type for {value}")),
            Some("object") => self.object(schema, value, at),
            Some("array") => self.array(schema, value, at),
            Some("string") => wants(value.is_string(), value, "a string", at),
            Some("boolean") => wants(value.is_boolean(), value, "a boolean", at),
            Some("integer") => {
                wants(value.is_i64() || value.is_u64(), value, "an integer", at)?;
                bounded(schema, value, at)
            }
            Some(other) => Err(format!("{at}: the contract holds no type {other}")),
        }
    }

    fn object(&self, schema: &Value, value: &Value, at: &str) -> Result<(), String> {
        let Some(held) = value.as_object() else {
            return Err(format!("{at}: {value} is no object"));
        };
        let empty = Vec::new();
        for name in schema
            .get("required")
            .and_then(Value::as_array)
            .unwrap_or(&empty)
        {
            let name = name.as_str().expect("a required name reads as a string");
            if !held.contains_key(name) {
                return Err(format!("{at}: the answer carries no {name}"));
            }
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
        for (name, entry) in held {
            match properties.and_then(|properties| properties.get(name)) {
                Some(property) => self.check(property, entry, &format!("{at}/{name}"))?,
                None if closed => {
                    return Err(format!("{at}: the contract declares no property {name}"));
                }
                None => {}
            }
        }
        Ok(())
    }

    fn array(&self, schema: &Value, value: &Value, at: &str) -> Result<(), String> {
        let Some(held) = value.as_array() else {
            return Err(format!("{at}: {value} is no array"));
        };
        let items = schema
            .get("items")
            .unwrap_or_else(|| panic!("{at}: an array of the contract names its items"));
        for (index, entry) in held.iter().enumerate() {
            self.check(items, entry, &format!("{at}/{index}"))?;
        }
        Ok(())
    }

    fn request(&self, path: &str) -> &Value {
        let schema = self.at(&[
            "paths",
            path,
            "post",
            "requestBody",
            "content",
            "application/json",
            "schema",
        ]);
        self.resolve(schema)
    }

    fn parameters(&self, path: &str) -> Vec<Parameter> {
        self.at(&["paths", path, "get", "parameters"])
            .as_array()
            .expect("the parameters of a route")
            .iter()
            .map(|parameter| Parameter {
                name: parameter["name"]
                    .as_str()
                    .expect("the name of a parameter")
                    .to_owned(),
                held: parameter["in"]
                    .as_str()
                    .expect("the place of a parameter")
                    .to_owned(),
                required: parameter["required"].as_bool().unwrap_or(false),
                schema: parameter["schema"].clone(),
            })
            .collect()
    }

    fn walk(&self, schema: &Value, sample: &Value, at: &str, walk: &mut Walk) {
        let schema = self.resolve(schema);
        match schema.get("type").and_then(Value::as_str) {
            Some("object") => {
                let none = Value::Array(Vec::new());
                let required = names(schema.get("required").unwrap_or(&none));
                let empty = serde_json::Map::new();
                let properties = schema
                    .get("properties")
                    .and_then(Value::as_object)
                    .unwrap_or(&empty);
                let held = sample.as_object().expect("an object of the sample");
                let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
                walk.nodes.push(Node {
                    pointer: at.to_owned(),
                    closed,
                });
                for name in properties.keys() {
                    if !held.contains_key(name) {
                        walk.missing.push(format!("{at}/{name}"));
                    }
                }
                for (name, entry) in held {
                    walk.probes.push(Probe {
                        pointer: at.to_owned(),
                        name: name.clone(),
                        required: required.contains(name),
                        closed,
                    });
                    if let Some(property) = properties.get(name) {
                        self.walk(property, entry, &format!("{at}/{name}"), walk);
                    }
                }
            }
            Some("array") => {
                if let Some(items) = schema.get("items") {
                    for (index, entry) in sample
                        .as_array()
                        .expect("an array of the sample")
                        .iter()
                        .enumerate()
                    {
                        self.walk(items, entry, &format!("{at}/{index}"), walk);
                    }
                }
            }
            _ => {}
        }
    }

    fn confront(&self, answer: &Answer) {
        let method = answer.call.method.to_ascii_lowercase();
        let declared = ROUTES.iter().any(|route| {
            route.path() == answer.call.path && route.method().as_str() == answer.call.method
        });
        let schema = if declared {
            let responses = self.at(&["paths", answer.call.path, &method, "responses"]);
            let response = responses.get(answer.status.to_string()).unwrap_or_else(|| {
                panic!(
                    "the contract declares no {} answer for {} {}",
                    answer.status, answer.call.method, answer.call.path
                )
            });
            self.resolve(response)["content"]["application/json"]["schema"].clone()
        } else {
            json!({"$ref": "#/components/schemas/Error"})
        };
        if let Err(reason) = self.check(&schema, &answer.json(), "") {
            panic!(
                "{} {} answered {}: {reason}",
                answer.call.method, answer.call.target, answer.status
            );
        }
    }
}

fn wants(held: bool, value: &Value, wanted: &str, at: &str) -> Result<(), String> {
    if held {
        Ok(())
    } else {
        Err(format!("{at}: {value} is not {wanted}"))
    }
}

fn bounded(schema: &Value, value: &Value, at: &str) -> Result<(), String> {
    let number = value
        .as_f64()
        .ok_or_else(|| format!("{at}: {value} carries no number a bound compares"))?;
    for (name, holds) in [("minimum", true), ("maximum", false)] {
        if let Some(bound) = schema.get(name).and_then(Value::as_f64)
            && (holds && number < bound || !holds && number > bound)
        {
            return Err(format!("{at}: {number} is outside the {name} {bound}"));
        }
    }
    Ok(())
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("a list of strings")
        .iter()
        .map(|entry| entry.as_str().expect("a string").to_owned())
        .collect()
}

#[test]
fn the_contract_and_the_server_hold_the_same_routes() {
    let contract = Contract::read();
    let declared: BTreeSet<String> = contract
        .at(&["paths"])
        .as_object()
        .expect("the paths of the contract")
        .iter()
        .flat_map(|(path, operations)| {
            let operations = operations.as_object().expect("the operations of a path");
            assert!(
                operations.keys().all(|key| METHODS.contains(&key.as_str())),
                "{path} carries what no test reads: {:?}",
                operations
                    .keys()
                    .filter(|key| !METHODS.contains(&key.as_str()))
                    .collect::<Vec<&String>>()
            );
            operations
                .keys()
                .map(move |method| format!("{} {path}", method.to_ascii_uppercase()))
        })
        .collect();
    let served: BTreeSet<String> = ROUTES
        .iter()
        .map(|route| format!("{} {}", route.method().as_str(), route.path()))
        .collect();

    assert_eq!(declared, served);
}

#[test]
fn every_answer_the_contract_declares_is_an_answer_the_server_can_name() {
    let contract = Contract::read();
    let mut held: BTreeSet<String> = Kind::ALL
        .iter()
        .map(|kind| kind.status().to_string())
        .collect();
    held.insert("200".to_owned());

    for route in ROUTES {
        let method = route.method().as_str().to_ascii_lowercase();
        let declared: BTreeSet<String> = contract
            .at(&["paths", route.path(), &method, "responses"])
            .as_object()
            .expect("the answers of an operation")
            .keys()
            .cloned()
            .collect();
        assert!(declared.contains("200"), "{}", route.path());
        assert!(
            declared.is_subset(&held),
            "{} declares answers no kind carries: {:?}",
            route.path(),
            &declared - &held
        );
    }
}

#[test]
fn every_route_names_the_command_line_tasks_it_serves() {
    let contract = Contract::read();
    let mut covered: BTreeSet<String> = BTreeSet::new();
    for route in ROUTES {
        let method = route.method().as_str().to_ascii_lowercase();
        let commands = strings(contract.at(&["paths", route.path(), &method, "x-cli"]));
        assert_eq!(
            commands,
            route.commands(),
            "{} {}",
            route.method().as_str(),
            route.path()
        );
        covered.extend(commands);
    }

    assert_eq!(
        covered,
        BTreeSet::from([
            "apply".to_owned(),
            "diff".to_owned(),
            "find".to_owned(),
            "ls".to_owned(),
            "plan".to_owned(),
            "sync".to_owned(),
        ])
    );
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("an object the server writes")
        .keys()
        .cloned()
        .collect()
}

fn written<Body: serde::Serialize>(body: &Body) -> Value {
    serde_json::to_value(body).expect("the server writes this answer")
}

fn answers() -> [(&'static str, Value, Value); 10] {
    [
        (
            "Item",
            written(&ItemBody {
                container: "shared/linux/0".to_owned(),
                placement: PlacementBody::Units(1),
                code: Some("jew".to_owned()),
                quality: "unique".to_owned(),
                fingerprint: FINGERPRINT.to_owned(),
            }),
            written(&ItemBody {
                container: "shared/linux/0".to_owned(),
                placement: PlacementBody::Units(1),
                code: None,
                quality: "unique".to_owned(),
                fingerprint: FINGERPRINT.to_owned(),
            }),
        ),
        (
            "Items",
            written(&ItemsBody { items: Vec::new() }),
            written(&ItemsBody { items: Vec::new() }),
        ),
        (
            "Difference",
            written(&DifferenceBody {
                container: "shared/linux/0".to_owned(),
                fingerprint: FINGERPRINT.to_owned(),
                before: 1,
                after: 0,
            }),
            written(&DifferenceBody {
                container: "shared/linux/0".to_owned(),
                fingerprint: FINGERPRINT.to_owned(),
                before: 1,
                after: 0,
            }),
        ),
        (
            "Differences",
            written(&DiffBody {
                differences: Vec::new(),
            }),
            written(&DiffBody {
                differences: Vec::new(),
            }),
        ),
        (
            "PlanAnswer",
            written(&PlanBody {
                plan: PlanDocument {
                    operations: Vec::new(),
                },
            }),
            written(&PlanBody {
                plan: PlanDocument {
                    operations: Vec::new(),
                },
            }),
        ),
        (
            "Plan",
            written(&PlanDocument {
                operations: Vec::new(),
            }),
            written(&PlanDocument {
                operations: Vec::new(),
            }),
        ),
        (
            "Operation",
            written(&operation(true)),
            written(&operation(false)),
        ),
        ("Source", written(&source(true)), written(&source(false))),
        (
            "Destination",
            written(&destination(true)),
            written(&destination(false)),
        ),
        (
            "Cell",
            written(&CellDocument { column: 1, row: 2 }),
            written(&CellDocument { column: 1, row: 2 }),
        ),
    ]
}

fn records() -> [(&'static str, Value, Value); 5] {
    [
        (
            "Realized",
            written(&RealizedBody {
                transfer: TransferDocument::Move,
                fingerprint: FINGERPRINT.to_owned(),
                from: "shared/linux/0".to_owned(),
                to: "shared/linux/1".to_owned(),
                landing: PlacementBody::Cell { column: 1, row: 2 },
            }),
            written(&RealizedBody {
                transfer: TransferDocument::Move,
                fingerprint: FINGERPRINT.to_owned(),
                from: "shared/linux/0".to_owned(),
                to: "shared/linux/1".to_owned(),
                landing: PlacementBody::Cell { column: 1, row: 2 },
            }),
        ),
        ("Duplication", written(&ledger()), written(&ledger())),
        ("ApplyAnswer", written(&applied()), written(&applied())),
        ("Error", refused(), refused()),
        (
            "ErrorDetail",
            refused()["error"].clone(),
            refused()["error"].clone(),
        ),
    ]
}

#[test]
fn every_answer_the_contract_declares_holds_the_properties_the_server_writes() {
    let contract = Contract::read();
    for (name, full, bare) in answers().into_iter().chain(records()) {
        let schema = contract.at(&["components", "schemas", name]);
        assert_eq!(
            keys(&full),
            names(&schema["properties"]),
            "{name}: the contract declares other properties than the server writes"
        );
        assert_eq!(
            keys(&bare),
            names(&schema["required"]),
            "{name}: the contract holds other properties for granted than the server always writes"
        );
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false)),
            "{name}: the contract leaves room for a property the server never writes"
        );
    }
}

#[test]
fn every_outcome_the_server_writes_is_an_outcome_the_contract_names() {
    let contract = Contract::read();
    covered(
        &contract,
        "SyncAnswer",
        &[
            written(&SyncBody::LockHeldElsewhere),
            written(&SyncBody::GameRunning),
            written(&SyncBody::GameStartedDuringSync),
            written(&SyncBody::NothingChanged),
            written(&SyncBody::Committed {
                commit: COMMIT.to_owned(),
                at: AT.to_owned(),
            }),
        ],
    );
    covered(
        &contract,
        "Record",
        &[
            written(&RecordBody::NothingChanged),
            written(&RecordBody::Committed {
                commit: COMMIT.to_owned(),
                at: AT.to_owned(),
            }),
        ],
    );
    covered(
        &contract,
        "Placement",
        &[
            written(&PlacementBody::Cell { column: 1, row: 2 }),
            written(&PlacementBody::Units(1)),
        ],
    );
}

fn covered(contract: &Contract, name: &str, held: &[Value]) {
    let schema = contract.at(&["components", "schemas", name]).clone();
    let branches = schema["oneOf"]
        .as_array()
        .expect("the branches of a choice");
    let mut taken: BTreeSet<usize> = BTreeSet::new();
    for value in held {
        let matched: Vec<usize> = branches
            .iter()
            .enumerate()
            .filter(|(_, branch)| contract.check(branch, value, name).is_ok())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            matched.len(),
            1,
            "{name}: {} branches hold {value}",
            matched.len()
        );
        taken.insert(matched[0]);
    }
    assert_eq!(
        taken.len(),
        branches.len(),
        "{name}: the contract holds a branch the server never writes"
    );
}

#[test]
fn the_bounds_of_the_contract_are_the_bounds_the_rules_hold() {
    let contract = Contract::read();
    for (schema, name) in [("Source", "units"), ("Duplication", "units")] {
        let bounds = contract.at(&["components", "schemas", schema, "properties", name]);
        assert_eq!(
            bounds["minimum"].as_u64(),
            Some(u64::from(PileUnits::ONE.get())),
            "{schema}/{name}"
        );
        assert_eq!(
            bounds["maximum"].as_u64(),
            Some(u64::from(PileCount::MAX)),
            "{schema}/{name}"
        );
    }
}

#[test]
fn the_defaults_of_the_contract_are_the_defaults_the_tasks_take() {
    let contract = Contract::read();
    let apply = contract.at(&["components", "schemas", "ApplyRequest", "properties"]);
    assert_eq!(apply["timer"]["x-default"].as_str(), Some(DEFAULT_TIMER));
    assert_eq!(apply["saves"]["x-default"].as_str(), Some(MIRROR_DIRECTORY));
}

#[test]
fn the_contract_serves_the_loopback_only() {
    let contract = Contract::read();
    let servers = contract
        .at(&["servers"])
        .as_array()
        .expect("the servers of the contract");
    assert!(!servers.is_empty());
    for held in servers {
        let url = held["url"].as_str().expect("a url");
        let host = url
            .trim_start_matches("http://")
            .split(':')
            .next()
            .expect("a host");
        assert_eq!(host, "127.0.0.1", "{url}");
        assert_eq!(
            url,
            format!("http://127.0.0.1:{}", server::DEFAULT_PORT),
            "the contract serves another port than the serve task takes"
        );
    }
}

#[test]
fn every_kind_the_server_answers_carries_the_status_the_contract_declares() {
    let contract = Contract::read();
    let declared: BTreeMap<String, u16> = contract
        .at(&["x-kind-status"])
        .as_object()
        .expect("the kinds of the contract")
        .iter()
        .map(|(kind, status)| {
            (
                kind.clone(),
                u16::try_from(status.as_u64().expect("a status")).expect("a status"),
            )
        })
        .collect();
    let held: BTreeMap<String, u16> = Kind::ALL
        .iter()
        .map(|kind| (kind.as_str().to_owned(), kind.status()))
        .collect();

    assert_eq!(declared, held);
    assert_eq!(
        strings(contract.at(&[
            "components",
            "schemas",
            "ErrorDetail",
            "properties",
            "kind",
            "enum"
        ]))
        .into_iter()
        .collect::<BTreeSet<String>>(),
        held.keys().cloned().collect::<BTreeSet<String>>()
    );
}

#[test]
fn the_qualities_and_the_transfers_of_the_contract_are_the_ones_the_server_answers() {
    let contract = Contract::read();
    assert_eq!(
        strings(contract.at(&["components", "schemas", "Quality", "enum"])),
        Quality::names()
    );
    assert_eq!(
        strings(contract.at(&["components", "schemas", "Transfer", "enum"])),
        [TransferDocument::Move, TransferDocument::Duplicate].map(|transfer| serde_json::to_value(
            transfer
        )
        .expect("a transfer writes")
        .as_str()
        .expect("a string")
        .to_owned())
    );
}

#[test]
fn the_body_limit_of_the_contract_is_the_one_the_server_holds() {
    let contract = Contract::read();
    assert_eq!(
        contract.at(&["x-request-body-limit-bytes"]).as_u64(),
        Some(server::BODY_LIMIT as u64)
    );
}

#[derive(Debug)]
struct Confrontation {
    contract: Contract,
    seen: BTreeSet<String>,
}

impl Confrontation {
    fn new() -> Self {
        Self {
            contract: Contract::read(),
            seen: BTreeSet::new(),
        }
    }

    fn holds(&mut self, answer: &Answer, status: u16) {
        assert_eq!(answer.status, status, "{}", answer.body);
        self.contract.confront(answer);
        self.seen.insert(format!(
            "{} {} {status}",
            answer.call.method, answer.call.path
        ));
    }

    fn answered(&self, routes: &[&str], status: u16) {
        for path in routes {
            let route = ROUTES
                .iter()
                .find(|route| route.path() == *path)
                .expect("a route of the server");
            assert!(
                self.seen
                    .contains(&format!("{} {path} {status}", route.method().as_str())),
                "{path} answered no {status} in this confrontation"
            );
        }
    }
}

fn corpus(rig: &Rig) -> (PathBuf, PathBuf) {
    let repo = repository(&rig.tree, "backup");
    let saves = repo.join("saves");
    seed(&saves);
    copy_fixture("v105/linux/Elektra.d2s", &saves.join("Elektra.d2s"));
    git(&repo, &["add", "-A", "--", "saves"]);
    git(&repo, &["commit", "-m", "saves"]);
    (repo, saves)
}

fn first_fingerprint(answer: &Answer) -> String {
    answer.json()["items"]
        .as_array()
        .expect("a list of items")
        .iter()
        .find(|item| item["container"] == "shared/linux/0")
        .expect("the first tab holds an item")["fingerprint"]
        .as_str()
        .expect("a fingerprint")
        .to_owned()
}

fn names(value: &Value) -> BTreeSet<String> {
    match value {
        Value::Array(entries) => entries
            .iter()
            .map(|entry| entry.as_str().expect("a name").to_owned())
            .collect(),
        Value::Object(fields) => fields.keys().cloned().collect(),
        other => panic!("{other} names nothing"),
    }
}

#[derive(Debug)]
struct Probe {
    pointer: String,
    name: String,
    required: bool,
    closed: bool,
}

#[derive(Debug)]
struct Node {
    pointer: String,
    closed: bool,
}

#[derive(Debug, Default)]
struct Walk {
    probes: Vec<Probe>,
    nodes: Vec<Node>,
    missing: Vec<String>,
}

#[derive(Debug)]
struct Parameter {
    name: String,
    held: String,
    required: bool,
    schema: Value,
}

fn without(sample: &Value, probe: &Probe) -> String {
    let mut short = sample.clone();
    short
        .pointer_mut(&probe.pointer)
        .expect("a node of the sample")
        .as_object_mut()
        .expect("an object of the sample")
        .remove(&probe.name);
    short.to_string()
}

fn renamed(sample: &Value, probe: &Probe) -> String {
    let mut other = sample.clone();
    let node = other
        .pointer_mut(&probe.pointer)
        .expect("a node of the sample")
        .as_object_mut()
        .expect("an object of the sample");
    let value = node.remove(&probe.name).expect("a property of the sample");
    node.insert("gold".to_owned(), value);
    other.to_string()
}

fn joined(fields: &[(&str, String)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<String>>()
        .join("&")
}

const OUTSIDE: &str = "outside-every-value-the-contract-names";
const FINGERPRINT: &str = "00000000000000000000000000000000";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn source(full: bool) -> SourceDocument {
    SourceDocument {
        container: "shared/linux/5".to_owned(),
        fingerprint: FINGERPRINT.to_owned(),
        units: full.then_some(2),
    }
}

fn destination(full: bool) -> DestinationDocument {
    DestinationDocument {
        container: "shared/linux/0".to_owned(),
        cell: full.then_some(CellDocument { column: 1, row: 2 }),
    }
}

fn operation(full: bool) -> OperationDocument {
    OperationDocument {
        transfer: TransferDocument::Move,
        from: source(full),
        to: destination(full),
    }
}

fn ledger() -> LedgerEntry {
    LedgerEntry {
        at: AT.to_owned(),
        fingerprint: FINGERPRINT.to_owned(),
        from: "shared/linux/0".to_owned(),
        to: "char/Elektra/cube".to_owned(),
        units: 1,
    }
}

fn refused() -> Value {
    serde_json::from_str(&Failure::new(Kind::BadRequest, "refused".to_owned()).body())
        .expect("the server writes this refusal")
}

fn applied() -> ApplyBody {
    ApplyBody {
        checkpoint: COMMIT.to_owned(),
        record: RecordBody::NothingChanged,
        realized: Vec::new(),
        duplications: Vec::new(),
        written: Vec::new(),
    }
}

fn plan_sample() -> Value {
    json!({"operations": [{
        "transfer": "move",
        "from": {
            "container": "shared/linux/5",
            "fingerprint": "00000000000000000000000000000000",
            "units": 2,
        },
        "to": {"container": "shared/linux/0", "cell": {"column": 1, "row": 2}},
    }]})
}

#[test]
fn every_request_the_contract_declares_is_the_request_the_server_reads() {
    let contract = Contract::read();
    let rig = Rig::new("contract-requests");
    let (repo, saves) = corpus(&rig);
    let samples: [(&'static str, Value); 3] = [
        (
            "/sync",
            json!({"source": saves, "repo": rig.tree.path("nowhere"),
                "lock": rig.tree.path("lock/backup.lock")}),
        ),
        ("/plan", json!({"saves": saves, "plan": plan_sample()})),
        (
            "/apply",
            json!({"repo": repo, "saves": saves, "plan": plan_sample(),
                "timer": ABSENT_TIMER}),
        ),
    ];

    for (path, sample) in samples {
        let schema = contract.request(path).clone();
        if let Err(reason) = contract.check(&schema, &sample, path) {
            panic!("the contract declares another request than the server reads: {reason}");
        }
        let mut walk = Walk::default();
        contract.walk(&schema, &sample, "", &mut walk);
        assert!(
            walk.missing.is_empty(),
            "{path}: the contract declares properties no request exercises: {:?}",
            walk.missing
        );

        let answer = rig.ask(Call::post(path, &sample.to_string()));
        assert_ne!(answer.status, 400, "{path}: {}", answer.body);

        for probe in &walk.probes {
            let at = format!("{path}{}/{}", probe.pointer, probe.name);
            let answer = rig.ask(Call::post(path, &renamed(&sample, probe)));
            if probe.required || probe.closed {
                assert_eq!(
                    answer.status, 400,
                    "the server reads {at} under a name the contract never declares"
                );
            } else {
                assert_ne!(answer.status, 400, "the server refuses to drop {at}");
            }

            let answer = rig.ask(Call::post(path, &without(&sample, probe)));
            if probe.required {
                assert_eq!(
                    answer.status, 400,
                    "the server reads a request without {at}"
                );
            } else {
                assert_ne!(
                    answer.status, 400,
                    "the server refuses a request without {at}"
                );
            }
        }

        for node in &walk.nodes {
            let mut extra = sample.clone();
            extra
                .pointer_mut(&node.pointer)
                .expect("a node of the sample")
                .as_object_mut()
                .expect("an object of the sample")
                .insert("gold".to_owned(), json!(1));
            let answer = rig.ask(Call::post(path, &extra.to_string()));
            let at = format!("{path}{}", node.pointer);
            if node.closed {
                assert_eq!(
                    answer.status, 400,
                    "the server reads a property {at} never declares"
                );
            } else {
                assert_ne!(
                    answer.status, 400,
                    "the server refuses a property {at} allows"
                );
            }
        }
    }
}

fn keywords(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Some(fields) = schema.as_object() else {
        return;
    };
    for (name, entry) in fields {
        if !KEYWORDS.contains(&name.as_str()) {
            found.push(format!("{at}/{name}"));
            continue;
        }
        match name.as_str() {
            "properties" => {
                for (property, entry) in entry.as_object().expect("the properties of a schema") {
                    keywords(entry, &format!("{at}/{property}"), found);
                }
            }
            "items" | "additionalProperties" => keywords(entry, &format!("{at}/{name}"), found),
            "oneOf" => {
                for (index, branch) in entry
                    .as_array()
                    .expect("the branches of a choice")
                    .iter()
                    .enumerate()
                {
                    keywords(branch, &format!("{at}/{index}"), found);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn the_contract_holds_no_keyword_the_confrontation_reads_past() {
    let contract = Contract::read();
    let mut found = Vec::new();
    for (name, schema) in contract
        .at(&["components", "schemas"])
        .as_object()
        .expect("the schemas of the contract")
    {
        keywords(schema, name, &mut found);
    }
    for route in ROUTES {
        if route.method() != Method::Get {
            continue;
        }
        for parameter in contract.parameters(route.path()) {
            keywords(
                &parameter.schema,
                &format!("{}?{}", route.path(), parameter.name),
                &mut found,
            );
        }
    }

    assert!(
        found.is_empty(),
        "the contract declares what no test reads: {found:?}"
    );
}

fn outside(
    contract: &Contract,
    rig: &Rig,
    route: &'static str,
    sample: &[(&str, String)],
    declared: &[Parameter],
) {
    for parameter in declared {
        if contract
            .resolve(&parameter.schema)
            .get("enum")
            .and_then(Value::as_array)
            .is_none_or(|allowed| allowed.contains(&json!(OUTSIDE)))
        {
            continue;
        }
        let held: Vec<(&str, String)> = sample
            .iter()
            .map(|(name, value)| {
                if *name == parameter.name {
                    (*name, OUTSIDE.to_owned())
                } else {
                    (*name, value.clone())
                }
            })
            .collect();
        let answer = rig.ask(Call::get(route, &joined(&held)));
        assert_eq!(
            answer.status, 400,
            "{route}: the server reads a {} the contract never names",
            parameter.name
        );
    }
}

#[test]
fn every_query_the_contract_declares_is_the_query_the_server_reads() {
    let contract = Contract::read();
    let rig = Rig::new("contract-queries");
    let saves = rig.tree.path("saves");
    seed(&saves);
    let path = encoded(&saves);
    let samples: [(&'static str, Vec<(&str, String)>); 2] = [
        (
            "/items",
            vec![
                ("saves", path.clone()),
                ("container", "shared/linux/0".to_owned()),
                ("code", "jew".to_owned()),
                ("quality", "unique".to_owned()),
                ("fingerprint", "0".to_owned()),
            ],
        ),
        (
            "/diff",
            vec![("saves", path.clone()), ("against", path.clone())],
        ),
    ];

    for (route, sample) in samples {
        let declared = contract.parameters(route);
        assert_eq!(
            declared
                .iter()
                .map(|parameter| parameter.name.clone())
                .collect::<BTreeSet<String>>(),
            sample
                .iter()
                .map(|(name, _)| (*name).to_owned())
                .collect::<BTreeSet<String>>(),
            "{route}: the contract and the query hold other parameters"
        );
        for parameter in &declared {
            assert_eq!(
                parameter.held, "query",
                "{route}: the server reads {} from the query alone",
                parameter.name
            );
            if parameter.name == "quality" {
                assert_eq!(
                    parameter.schema["$ref"].as_str(),
                    Some("#/components/schemas/Quality"),
                    "{route}: the server keeps one quality only"
                );
            }
            let (_, value) = sample
                .iter()
                .find(|(name, _)| *name == parameter.name)
                .expect("a parameter of the query");
            if let Err(reason) = contract.check(&parameter.schema, &json!(value), &parameter.name) {
                panic!(
                    "{route}: the query carries another value than the contract declares: {reason}"
                );
            }
        }

        let answer = rig.ask(Call::get(route, &joined(&sample)));
        assert_ne!(answer.status, 400, "{route}: {}", answer.body);

        for parameter in &declared {
            let short: Vec<(&str, String)> = sample
                .iter()
                .filter(|(name, _)| *name != parameter.name)
                .cloned()
                .collect();
            let answer = rig.ask(Call::get(route, &joined(&short)));
            let at = format!("{route}?{}", parameter.name);
            if parameter.required {
                assert_eq!(answer.status, 400, "the server reads a query without {at}");
            } else {
                assert_ne!(
                    answer.status, 400,
                    "the server refuses a query without {at}"
                );
            }
        }

        let mut extra = sample.clone();
        extra.push(("gold", "1".to_owned()));
        let answer = rig.ask(Call::get(route, &joined(&extra)));
        assert_eq!(
            answer.status, 400,
            "{route}: the server reads a parameter the contract never declares"
        );

        outside(&contract, &rig, route, &sample, &declared);
    }
}

#[test]
fn the_reading_routes_answer_the_schemas_the_contract_declares() {
    let mut confrontation = Confrontation::new();
    let rig = Rig::new("contract-reads");
    let saves = rig.tree.path("saves");
    seed(&saves);
    let path = encoded(&saves);

    confrontation.holds(&rig.ask(Call::get("/items", &format!("saves={path}"))), 200);
    confrontation.holds(
        &rig.ask(Call::get(
            "/items",
            &format!("saves={}", encoded(&rig.tree.path("nowhere"))),
        )),
        404,
    );
    confrontation.holds(&rig.ask(Call::get("/items", "")), 400);
    confrontation.holds(
        &rig.ask(
            Call::get("/items", &format!("saves={path}")).with_origin("https://example.invalid"),
        ),
        403,
    );
    confrontation.holds(&rig.ask(Call::get("/items", "").spoken_by("PUT")), 405);
    confrontation.holds(&rig.ask(Call::get("/objects", "")), 404);

    let legacy = rig.tree.directory("legacy");
    copy_fixture("legacy/SharedStashHardCoreV2.d2i", &legacy.join("old.d2i"));
    confrontation.holds(
        &rig.ask(Call::get("/items", &format!("saves={}", encoded(&legacy)))),
        422,
    );

    let other = rig.tree.path("other");
    seed(&other);
    std::fs::remove_file(other.join("Gous.d2s")).expect("the character leaves");
    let answer = rig.ask(Call::get(
        "/diff",
        &format!("saves={path}&against={}", encoded(&other)),
    ));
    assert!(
        !answer.json()["differences"]
            .as_array()
            .expect("a list of differences")
            .is_empty(),
        "the comparison holds no difference to confront"
    );
    confrontation.holds(&answer, 200);

    confrontation.answered(&["/items", "/diff"], 200);
}

#[test]
fn the_writing_routes_answer_the_schemas_the_contract_declares() {
    let mut confrontation = Confrontation::new();
    let rig = Rig::new("contract-writes");
    let (repo, saves) = corpus(&rig);
    let listing = rig.ask(Call::get("/items", &format!("saves={}", encoded(&saves))));
    let fingerprint = first_fingerprint(&listing);
    let plan = json!({"operations": [{
        "transfer": "duplicate",
        "from": {"container": "shared/linux/0", "fingerprint": fingerprint},
        "to": {"container": "char/Elektra/cube"},
    }]});
    let refused = json!({"operations": [{
        "transfer": "move",
        "from": {"container": "shared/linux/0", "fingerprint": fingerprint},
        "to": {"container": "shared/linux/5"},
    }]});

    confrontation.holds(
        &rig.ask(Call::post(
            "/plan",
            &json!({"saves": saves, "plan": plan}).to_string(),
        )),
        200,
    );
    confrontation.holds(
        &rig.ask(Call::post(
            "/plan",
            &json!({"saves": saves, "plan": refused, "gold": 1}).to_string(),
        )),
        400,
    );
    confrontation.holds(
        &rig.ask(Call::post(
            "/plan",
            &format!("{{\"saves\":\"{}\"}}", "x".repeat(server::BODY_LIMIT)),
        )),
        413,
    );
    confrontation.holds(
        &rig.ask(Call::post(
            "/apply",
            &json!({"repo": repo, "timer": ABSENT_TIMER, "plan": refused}).to_string(),
        )),
        422,
    );
    let answer = rig.ask(Call::post(
        "/apply",
        &json!({"repo": repo, "timer": ABSENT_TIMER, "plan": plan}).to_string(),
    ));
    assert!(
        !answer.json()["duplications"]
            .as_array()
            .expect("a list of duplications")
            .is_empty(),
        "the apply recorded no duplication to confront"
    );
    confrontation.holds(&answer, 200);

    let source = rig.tree.path("source");
    seed(&source);
    let backup = repository(&rig.tree, "mirror");
    let lock = rig.tree.path("lock/backup.lock");
    confrontation.holds(
        &rig.ask(Call::post(
            "/sync",
            &json!({"source": source, "repo": backup, "lock": lock}).to_string(),
        )),
        200,
    );
    confrontation.holds(
        &rig.ask(Call::post(
            "/sync",
            &json!({"source": rig.tree.path("nowhere"), "repo": backup, "lock": lock}).to_string(),
        )),
        409,
    );

    confrontation.answered(&["/plan", "/apply", "/sync"], 200);
}
