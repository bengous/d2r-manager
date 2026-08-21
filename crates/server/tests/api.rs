mod support;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use app::ledger::{LEDGER_DIRECTORY, LEDGER_FILE};
use app::sync::GAME_PROCESS;

use support::{
    ABSENT_TIMER, AT, Call, Rig, STASH, bytes_of, copy_fixture, encoded, git, repository, seed,
};

fn saves(rig: &Rig) -> PathBuf {
    let saves = rig.tree.path("saves");
    seed(&saves);
    saves
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

fn items(rig: &Rig, saves: &Path, filters: &str) -> Value {
    let query = format!("saves={}{filters}", encoded(saves));
    let answer = rig.ask(Call::get("/items", &query));
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()
}

fn first_fingerprint(rig: &Rig, saves: &Path, container: &str) -> String {
    let listing = items(rig, saves, &format!("&container={container}"));
    listing["items"][0]["fingerprint"]
        .as_str()
        .expect("the container holds an item")
        .to_owned()
}

fn move_to(from: &str, fingerprint: &str, to: &str) -> Value {
    json!({"operations": [{
        "transfer": "move",
        "from": {"container": from, "fingerprint": fingerprint},
        "to": {"container": to},
    }]})
}

fn duplicate_to(from: &str, fingerprint: &str, to: &str) -> Value {
    json!({"operations": [{
        "transfer": "duplicate",
        "from": {"container": from, "fingerprint": fingerprint},
        "to": {"container": to},
    }]})
}

fn apply(rig: &Rig, repo: &Path, plan: &Value) -> support::Answer {
    rig.ask(Call::post(
        "/apply",
        &json!({"repo": repo, "timer": ABSENT_TIMER, "plan": plan}).to_string(),
    ))
}

#[test]
fn the_api_lists_every_item_the_saves_hold() {
    let rig = Rig::new("items");
    let saves = saves(&rig);

    let listing = items(&rig, &saves, "");
    let held = listing["items"].as_array().expect("a list of items");
    assert_eq!(held.len(), 262);
    assert!(held.iter().all(|item| {
        let container = item["container"].as_str().expect("a container");
        container.starts_with("shared/linux/") || container.starts_with("char/Gous/")
    }));
}

#[test]
fn the_api_keeps_the_one_container_the_query_names() {
    let rig = Rig::new("items-one");
    let saves = saves(&rig);

    let listing = items(&rig, &saves, "&container=char/Gous/inventory");
    let held = listing["items"].as_array().expect("a list of items");
    assert!(!held.is_empty());
    assert!(
        held.iter()
            .all(|item| item["container"] == "char/Gous/inventory")
    );
}

#[test]
fn the_api_keeps_the_code_and_the_quality_the_query_names() {
    let rig = Rig::new("items-find");
    let saves = saves(&rig);

    let listing = items(&rig, &saves, "&code=cm1&quality=magic");
    let held = listing["items"].as_array().expect("a list of items");
    assert!(!held.is_empty());
    assert!(
        held.iter()
            .all(|item| item["code"] == "cm1" && item["quality"] == "magic")
    );
}

#[test]
fn a_comparison_of_a_directory_with_itself_holds_no_difference() {
    let rig = Rig::new("diff");
    let saves = saves(&rig);
    let path = encoded(&saves);

    let answer = rig.ask(Call::get("/diff", &format!("saves={path}&against={path}")));
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(answer.json()["differences"], json!([]));
}

#[test]
fn a_comparison_after_an_apply_names_the_containers_the_item_left_and_took() {
    let rig = Rig::new("diff-after");
    let (repo, saves) = corpus(&rig);
    let before = rig.tree.path("before");
    seed(&before);
    copy_fixture("v105/linux/Elektra.d2s", &before.join("Elektra.d2s"));

    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let answer = apply(
        &rig,
        &repo,
        &move_to("shared/linux/0", &fingerprint, "shared/linux/2"),
    );
    assert_eq!(answer.status, 200, "{}", answer.body);

    let answer = rig.ask(Call::get(
        "/diff",
        &format!("saves={}&against={}", encoded(&before), encoded(&saves)),
    ));
    let differences = answer.json();
    let moved: Vec<&Value> = differences["differences"]
        .as_array()
        .expect("a list of differences")
        .iter()
        .filter(|difference| difference["fingerprint"] == fingerprint.as_str())
        .collect();
    assert_eq!(moved.len(), 2, "{differences}");
    assert!(
        moved
            .iter()
            .any(|difference| difference["container"] == "shared/linux/0"
                && difference["before"] == 1
                && difference["after"] == 0)
    );
    assert!(
        moved
            .iter()
            .any(|difference| difference["container"] == "shared/linux/2"
                && difference["before"] == 0
                && difference["after"] == 1)
    );
}

#[test]
fn a_plan_answers_its_normal_form_and_leaves_every_byte_where_it_found_it() {
    let rig = Rig::new("plan");
    let saves = saves(&rig);
    let held = bytes_of(&saves);

    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let answer = rig.ask(Call::post(
        "/plan",
        &json!({
            "saves": saves,
            "plan": move_to("shared/linux/0", &fingerprint, "shared/linux/1"),
        })
        .to_string(),
    ));

    assert_eq!(answer.status, 200, "{}", answer.body);
    let plan = answer.json();
    assert_eq!(plan["plan"]["operations"][0]["transfer"], "move");
    assert_eq!(
        plan["plan"]["operations"][0]["from"]["fingerprint"],
        fingerprint.as_str()
    );
    assert_eq!(
        plan["plan"]["operations"][0]["to"]["container"],
        "shared/linux/1"
    );
    assert_eq!(bytes_of(&saves), held);
}

#[test]
fn a_plan_the_rules_refuse_answers_a_refusal_and_no_plan() {
    let rig = Rig::new("plan-refused");
    let saves = saves(&rig);

    let answer = rig.ask(Call::post(
        "/plan",
        &json!({
            "saves": saves,
            "plan": move_to(
                "shared/linux/0",
                "00000000000000000000000000000000",
                "shared/linux/1",
            ),
        })
        .to_string(),
    ));

    assert_eq!(answer.status, 422, "{}", answer.body);
    assert_eq!(answer.kind(), "rule_refused");
    assert!(answer.body.contains("fingerprint"), "{}", answer.body);
}

#[test]
fn a_transfer_the_plan_document_never_holds_is_refused_before_the_rules() {
    let rig = Rig::new("plan-create");
    let saves = saves(&rig);

    let answer = rig.ask(Call::post(
        "/plan",
        &json!({
            "saves": saves,
            "plan": {"operations": [{
                "transfer": "create",
                "from": {"container": "shared/linux/0", "fingerprint": "00000000000000000000000000000000"},
                "to": {"container": "shared/linux/1"},
            }]},
        })
        .to_string(),
    ));

    assert_eq!(answer.status, 400, "{}", answer.body);
    assert_eq!(answer.kind(), "bad_request");
}

#[test]
fn an_apply_writes_the_saves_reads_them_back_and_commits_the_plan() {
    let rig = Rig::new("apply");
    let (repo, saves) = corpus(&rig);

    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let answer = apply(
        &rig,
        &repo,
        &move_to("shared/linux/0", &fingerprint, "shared/linux/2"),
    );

    assert_eq!(answer.status, 200, "{}", answer.body);
    let applied = answer.json();
    assert_eq!(applied["record"]["outcome"], "committed");
    assert_eq!(applied["record"]["at"], AT);
    assert_eq!(applied["realized"][0]["transfer"], "move");
    assert_eq!(applied["realized"][0]["from"], "shared/linux/0");
    assert_eq!(applied["realized"][0]["to"], "shared/linux/2");
    assert_eq!(applied["duplications"], json!([]));
    assert_eq!(
        applied["written"],
        json!([saves.join(format!("{STASH}.d2i")).to_string_lossy()])
    );
    assert_eq!(rig.timers.called(), ["state", "pause", "resume"]);

    assert_eq!(
        git(&repo, &["rev-parse", "HEAD"]),
        applied["record"]["commit"].as_str().expect("a commit")
    );
    assert!(
        git(&repo, &["log", "-1", "--format=%B"]).contains("\"transfer\": \"move\""),
        "the commit message carries no plan"
    );
    assert!(
        items(&rig, &saves, "&container=shared/linux/2")["items"]
            .as_array()
            .expect("a list of items")
            .iter()
            .any(|item| item["fingerprint"] == fingerprint.as_str())
    );
}

#[test]
fn a_duplication_answers_the_ledger_line_the_commit_holds() {
    let rig = Rig::new("apply-ledger");
    let (repo, saves) = corpus(&rig);

    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let answer = apply(
        &rig,
        &repo,
        &duplicate_to("shared/linux/0", &fingerprint, "char/Elektra/cube"),
    );

    assert_eq!(answer.status, 200, "{}", answer.body);
    let applied = answer.json();
    assert_eq!(
        applied["duplications"],
        json!([{
            "at": AT,
            "fingerprint": fingerprint,
            "from": "shared/linux/0",
            "to": "char/Elektra/cube",
            "units": 1,
        }])
    );

    let ledger = repo.join(LEDGER_DIRECTORY).join(LEDGER_FILE);
    let line = std::fs::read_to_string(&ledger).expect("the ledger exists");
    assert_eq!(
        serde_json::from_str::<Value>(line.trim_end()).expect("the line reads back"),
        applied["duplications"][0]
    );
    let mut names: Vec<String> = git(&repo, &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            format!("{LEDGER_DIRECTORY}/{LEDGER_FILE}"),
            "saves/Elektra.d2s".to_owned(),
        ]
    );
}

#[test]
fn a_move_from_a_grid_to_an_advanced_stash_tab_is_refused_before_any_write() {
    let rig = Rig::new("apply-advanced");
    let (repo, saves) = corpus(&rig);
    let held = bytes_of(&saves);
    let commit = git(&repo, &["rev-parse", "HEAD"]);

    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let answer = apply(
        &rig,
        &repo,
        &move_to("shared/linux/0", &fingerprint, "shared/linux/5"),
    );

    assert_eq!(answer.status, 422, "{}", answer.body);
    assert_eq!(answer.kind(), "projection_refused");
    assert_eq!(bytes_of(&saves), held);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), commit);
    assert!(git(&repo, &["status", "--porcelain"]).is_empty());
    assert!(rig.timers.called().is_empty());
}

#[test]
fn two_applies_at_once_run_one_after_the_other() {
    let rig = Rig::new("apply-together");
    let (repo, saves) = corpus(&rig);
    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let plan = json!({"repo": repo, "timer": ABSENT_TIMER,
        "plan": move_to("shared/linux/0", &fingerprint, "shared/linux/2")})
    .to_string();

    let answers = rig.ask_together(Call::post("/apply", &plan), Call::post("/apply", &plan));

    let mut statuses: Vec<u16> = answers.iter().map(|answer| answer.status).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, [200, 422], "{:?}", answers.map(|a| a.body));
    assert_eq!(
        items(&rig, &saves, &format!("&fingerprint={fingerprint}"))["items"]
            .as_array()
            .expect("a list of items")
            .len(),
        1
    );
    assert_eq!(git(&repo, &["log", "--oneline"]).lines().count(), 3);
    assert!(git(&repo, &["status", "--porcelain"]).is_empty());
}

#[test]
fn an_apply_while_the_game_runs_is_refused() {
    let rig = Rig::new("apply-game");
    let (repo, saves) = corpus(&rig);
    let fingerprint = first_fingerprint(&rig, &saves, "shared/linux/0");
    let held = bytes_of(&saves);
    rig.processes.sees(&format!("-x {GAME_PROCESS}"));

    let answer = apply(
        &rig,
        &repo,
        &move_to("shared/linux/0", &fingerprint, "shared/linux/2"),
    );

    assert_eq!(answer.status, 409, "{}", answer.body);
    assert_eq!(answer.kind(), "game_running");
    assert_eq!(bytes_of(&saves), held);
}

#[test]
fn a_sync_mirrors_the_saves_and_commits_them() {
    let rig = Rig::new("sync");
    let source = saves(&rig);
    let repo = repository(&rig.tree, "backup");
    let body = json!({
        "source": source,
        "repo": repo,
        "lock": rig.tree.path("lock/backup.lock"),
    })
    .to_string();

    let answer = rig.ask(Call::post("/sync", &body));
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(answer.json()["outcome"], "committed");
    assert_eq!(answer.json()["at"], AT);
    assert!(repo.join(format!("saves/{STASH}.d2i")).is_file());
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s"]),
        format!("backup: {AT}")
    );

    let answer = rig.ask(Call::post("/sync", &body));
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(
        answer.json()["outcome"],
        "nothing_changed",
        "the lock of the first sync outlived it"
    );
}

#[test]
fn a_sync_of_a_directory_that_is_absent_is_refused() {
    let rig = Rig::new("sync-absent");
    let repo = repository(&rig.tree, "backup");

    let answer = rig.ask(Call::post(
        "/sync",
        &json!({
            "source": rig.tree.path("nowhere"),
            "repo": repo,
            "lock": rig.tree.path("lock/backup.lock"),
        })
        .to_string(),
    ));

    assert_eq!(answer.status, 409, "{}", answer.body);
    assert_eq!(answer.kind(), "path_refused");
}

#[test]
fn a_listing_of_a_directory_that_is_absent_is_refused() {
    let rig = Rig::new("items-absent");
    let answer = rig.ask(Call::get(
        "/items",
        &format!("saves={}", encoded(&rig.tree.path("nowhere"))),
    ));

    assert_eq!(answer.status, 404, "{}", answer.body);
    assert_eq!(answer.kind(), "not_found");
}

#[test]
fn a_save_the_version_105_reader_refuses_is_no_internal_failure() {
    let rig = Rig::new("items-legacy");
    let saves = rig.tree.directory("saves");
    copy_fixture("legacy/SharedStashHardCoreV2.d2i", &saves.join("old.d2i"));

    let answer = rig.ask(Call::get("/items", &format!("saves={}", encoded(&saves))));

    assert_eq!(answer.status, 422, "{}", answer.body);
    assert_eq!(answer.kind(), "unsupported_save");
}

#[test]
fn a_route_this_api_never_serves_is_no_route() {
    let rig = Rig::new("route");
    let answer = rig.ask(Call::get("/objects", ""));

    assert_eq!(answer.status, 404, "{}", answer.body);
    assert_eq!(answer.kind(), "no_such_route");
}

#[test]
fn a_method_a_route_never_answers_is_refused() {
    let rig = Rig::new("method");
    let answer = rig.ask(Call::get("/items", "").spoken_by("DELETE"));

    assert_eq!(answer.status, 405, "{}", answer.body);
    assert_eq!(answer.kind(), "method_not_allowed");
}

#[test]
fn a_parameter_the_route_never_takes_is_refused() {
    let rig = Rig::new("parameter");
    let saves = saves(&rig);
    let answer = rig.ask(Call::get(
        "/items",
        &format!("saves={}&gold=1", encoded(&saves)),
    ));

    assert_eq!(answer.status, 400, "{}", answer.body);
    assert_eq!(answer.kind(), "bad_request");
}

#[test]
fn a_body_above_the_limit_is_refused() {
    let rig = Rig::new("body");
    let body = format!("{{\"saves\":\"{}\"}}", "x".repeat(server::BODY_LIMIT));
    let answer = rig.ask(Call::post("/plan", &body));

    assert_eq!(answer.status, 413, "{}", answer.body);
    assert_eq!(answer.kind(), "payload_too_large");
}

#[test]
fn a_request_from_another_machine_is_refused() {
    let rig = Rig::new("origin");
    let saves = saves(&rig);
    let query = format!("saves={}", encoded(&saves));

    let answer = rig.ask(Call::get("/items", &query).with_origin("https://example.invalid"));
    assert_eq!(answer.status, 403, "{}", answer.body);
    assert_eq!(answer.kind(), "forbidden_origin");

    let answer = rig.ask(Call::get("/items", &query).with_origin("http://127.0.0.1:7105"));
    assert_eq!(answer.status, 200, "{}", answer.body);
}

#[test]
fn the_server_listens_on_the_loopback_only() {
    let rig = Rig::new("loopback");
    assert!(rig.address().ip().is_loopback(), "{}", rig.address());
    assert_ne!(rig.address().port(), 0);
}

#[test]
fn every_answer_carries_json() {
    let rig = Rig::new("content-type");
    let answer = rig.ask(Call::get("/items", ""));

    assert_eq!(answer.status, 400, "{}", answer.body);
    assert_eq!(answer.content_type, "application/json");
}
