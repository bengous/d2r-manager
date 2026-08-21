mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use app::inspect::Quality;
use app::transaction::TransferDocument;
use server::failure::Kind;
use server::route::ROUTES;

use support::{ABSENT_TIMER, Answer, Call, Rig, copy_fixture, encoded, git, repository, seed};

const CONTRACT: &str = include_str!("../../../docs/openapi.json");

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
        match schema.get("type").and_then(Value::as_str) {
            None => Ok(()),
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
                required: parameter["required"]
                    .as_bool()
                    .expect("the requiredness of a parameter"),
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
    let number = value.as_i64().unwrap_or_default();
    for (name, holds) in [("minimum", true), ("maximum", false)] {
        if let Some(bound) = schema.get(name).and_then(Value::as_i64)
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
            operations
                .as_object()
                .expect("the operations of a path")
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

#[test]
fn the_contract_serves_the_loopback_only() {
    let contract = Contract::read();
    let servers = contract
        .at(&["servers"])
        .as_array()
        .expect("the servers of the contract");
    assert!(!servers.is_empty());
    for server in servers {
        let url = server["url"].as_str().expect("a url");
        let host = url
            .trim_start_matches("http://")
            .split(':')
            .next()
            .expect("a host");
        assert_eq!(host, "127.0.0.1", "{url}");
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
}

#[derive(Debug, Default)]
struct Walk {
    probes: Vec<Probe>,
    missing: Vec<String>,
}

#[derive(Debug)]
struct Parameter {
    name: String,
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

fn joined(fields: &[(&str, String)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<String>>()
        .join("&")
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
    let saves = rig.tree.path("saves");
    seed(&saves);
    let nowhere = rig.tree.path("nowhere");
    let samples: [(&'static str, Value); 3] = [
        (
            "/sync",
            json!({"source": saves, "repo": nowhere, "lock": rig.tree.path("lock/backup.lock")}),
        ),
        ("/plan", json!({"saves": saves, "plan": plan_sample()})),
        (
            "/apply",
            json!({"repo": nowhere, "saves": saves, "plan": plan_sample(),
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
            let answer = rig.ask(Call::post(path, &without(&sample, probe)));
            let at = format!("{path}{}/{}", probe.pointer, probe.name);
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

        let mut extra = sample.clone();
        extra["gold"] = json!(1);
        let answer = rig.ask(Call::post(path, &extra.to_string()));
        assert_eq!(
            answer.status, 400,
            "{path}: the server reads a property the contract never declares"
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
    confrontation.holds(
        &rig.ask(Call::get("/diff", &format!("saves={path}&against={path}"))),
        200,
    );

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
        "transfer": "move",
        "from": {"container": "shared/linux/0", "fingerprint": fingerprint},
        "to": {"container": "shared/linux/2"},
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
    confrontation.holds(
        &rig.ask(Call::post(
            "/apply",
            &json!({"repo": repo, "timer": ABSENT_TIMER, "plan": plan}).to_string(),
        )),
        200,
    );

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
