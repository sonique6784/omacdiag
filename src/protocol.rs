use crate::{
    discovery,
    engine::{self, Answer, EventSink, OperatorAction, RunContext},
    model::*,
    planner, process, report,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

pub fn emit_raw(value: Value) {
    let mut stdout = io::stdout().lock();
    let _ = serde_json::to_writer(&mut stdout, &value);
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,
    pub command: String,
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub readiness: BTreeMap<String, Readiness>,
    pub selected_tests: Option<BTreeSet<String>>,
    pub microphone_id: Option<String>,
    #[serde(default)]
    pub acknowledged_preflight: bool,
    pub test_id: Option<String>,
    pub prompt_id: Option<String>,
    pub stage: Option<String>,
    pub verdict: Option<Outcome>,
    pub note: Option<String>,
    pub details: Option<Value>,
    pub path: Option<PathBuf>,
}
#[derive(Clone)]
struct Prompt {
    test_id: String,
    stage: String,
    id: String,
}
pub fn session(output: Option<PathBuf>) -> Result<(), String> {
    let inventory = discovery::discover();
    let plan = planner::build(
        &inventory,
        Profile::Quick,
        Mode::Guided,
        &BTreeMap::new(),
        None,
    );
    let report = Arc::new(Mutex::new(Report::new(inventory, plan)));
    let id = report.lock().unwrap().session_id.clone();
    let output = output.unwrap_or_else(|| engine::default_output(&report.lock().unwrap()));
    let emit: EventSink = Arc::new(move |kind, payload| {
        emit_raw(
            json!({"protocol_version":PROTOCOL_VERSION,"session_id":id,"request_id":null,"type":kind,"payload":payload}),
        )
    });
    emit(
        "hello",
        json!({"application_version":env!("CARGO_PKG_VERSION"),"preparation":planner::preparation(),"report_path":output}),
    );
    emit("inventory", json!(report.lock().unwrap().inventory));
    emit("plan", json!(report.lock().unwrap().plan));
    report::write_atomic(&output, &report.lock().unwrap())?;
    let cancel = Arc::new(AtomicBool::new(false));
    let prompt = Arc::new(Mutex::new(None::<Prompt>));
    let (replies, answers) = mpsc::channel::<OperatorAction>();
    let answers = Arc::new(Mutex::new(answers));
    let mut worker: Option<thread::JoinHandle<()>> = None;
    let (input_tx, input_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = io::stdin().lock();
        loop {
            let mut line = Vec::new();
            match Read::by_ref(&mut reader)
                .take(64 * 1024 + 1)
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > 64 * 1024 => {
                    let _ = input_tx.send(Err("Request exceeds 64 KiB".into()));
                    break;
                }
                Ok(_) => {
                    let parsed =
                        serde_json::from_slice::<Request>(&line).map_err(|e| e.to_string());
                    if input_tx.send(parsed).is_err() {
                        break;
                    }
                }
            }
        }
    });
    loop {
        if process::INTERRUPTED.load(Ordering::Relaxed) {
            cancel.store(true, Ordering::Relaxed);
            break;
        }
        let request = match input_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                emit("error", json!({"message":e}));
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => {
                cancel.store(true, Ordering::Relaxed);
                break;
            }
        };
        let active = worker.as_ref().is_some_and(|w| !w.is_finished());
        let session_id = report.lock().unwrap().session_id.clone();
        let respond = |kind: &str, payload: Value| {
            emit_raw(
                json!({"protocol_version":PROTOCOL_VERSION,"session_id":session_id,"request_id":request.request_id,"type":kind,"payload":payload}),
            );
        };
        if request.protocol_version != PROTOCOL_VERSION {
            respond("error", json!({"message":"Unsupported protocol version"}));
            continue;
        }
        match request.command.as_str() {
            "refresh" => {
                let can_refresh = {
                    let r = report.lock().unwrap();
                    r.state == "prepared" && r.results.is_empty()
                };
                if active || !can_refresh {
                    respond(
                        "error",
                        json!({"message":"Inventory can only be refreshed before testing"}),
                    );
                    continue;
                }
                let inventory = discovery::discover();
                let (profile, mode) = {
                    let r = report.lock().unwrap();
                    (r.plan.profile, r.plan.mode)
                };
                if let Err(error) =
                    planner::validate(&inventory, profile, mode, &request.readiness, None)
                {
                    respond("error", json!({"message":error}));
                    continue;
                }
                let plan = planner::build(&inventory, profile, mode, &request.readiness, None);
                {
                    let mut r = report.lock().unwrap();
                    r.inventory = inventory;
                    r.plan = plan;
                    r.refresh();
                }
                report::write_atomic(&output, &report.lock().unwrap())?;
                let r = report.lock().unwrap();
                respond("refreshed", json!({"inventory":r.inventory,"plan":r.plan}));
            }
            "prepare" => {
                if active {
                    respond(
                        "error",
                        json!({"message":"Cancel the active session before preparing another plan"}),
                    );
                    continue;
                }
                let validation = planner::validate(
                    &report.lock().unwrap().inventory,
                    request.profile,
                    request.mode,
                    &request.readiness,
                    request.selected_tests.as_ref(),
                );
                if let Err(error) = validation {
                    respond("error", json!({"message":error}));
                    continue;
                }
                let plan = {
                    let r = report.lock().unwrap();
                    planner::build(
                        &r.inventory,
                        request.profile,
                        request.mode,
                        &request.readiness,
                        request.selected_tests.as_ref(),
                    )
                };
                {
                    let mut r = report.lock().unwrap();
                    if !r.results.is_empty() {
                        respond_unlocked(
                            &request,
                            &r.session_id,
                            "error",
                            json!({"message":"Start a new session process to create another run; results are preserved"}),
                        );
                        continue;
                    }
                    r.plan = plan;
                    r.refresh();
                }
                report::write_atomic(&output, &report.lock().unwrap())?;
                respond("plan", json!(report.lock().unwrap().plan));
            }
            "start" => {
                if active || report.lock().unwrap().state != "prepared" {
                    respond("error", json!({"message":"Session already started"}));
                    continue;
                }
                if !request.acknowledged_preflight {
                    respond(
                        "error",
                        json!({"message":"Review equipment and acknowledge preflight before starting"}),
                    );
                    continue;
                }
                cancel.store(false, Ordering::Relaxed);
                let (p, e, c, a) = (
                    prompt.clone(),
                    emit.clone(),
                    cancel.clone(),
                    answers.clone(),
                );
                let answer: Answer = Arc::new(move |test, stage, result, _inventory| {
                    let id = report::session_id() + "-" + stage;
                    *p.lock().unwrap() = Some(Prompt {
                        test_id: test.id.clone(),
                        stage: stage.into(),
                        id: id.clone(),
                    });
                    e(
                        "prompt",
                        json!({"test":test,"stage":stage,"prompt_id":id,"result":result}),
                    );
                    loop {
                        if c.load(Ordering::Relaxed) || process::INTERRUPTED.load(Ordering::Relaxed)
                        {
                            *p.lock().unwrap() = None;
                            return None;
                        }
                        match a.lock().unwrap().recv_timeout(Duration::from_millis(100)) {
                            Ok(o) => {
                                *p.lock().unwrap() = None;
                                return Some(o);
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(_) => return None,
                        }
                    }
                });
                let ctx = RunContext {
                    report: report.clone(),
                    output: output.clone(),
                    cancel: cancel.clone(),
                    emit: emit.clone(),
                    answer,
                    microphone_id: request.microphone_id,
                };
                let e = emit.clone();
                worker = Some(thread::spawn(move || {
                    if let Err(error) = engine::run(ctx) {
                        e("error", json!({"message":error,"fatal":true}));
                    }
                }));
                respond("started", json!({}));
            }
            "repeat" => {
                if active
                    || !matches!(
                        report.lock().unwrap().state.as_str(),
                        "completed" | "cancelled"
                    )
                {
                    respond(
                        "error",
                        json!({"message":"Finish or cancel the session before repeating a check"}),
                    );
                    continue;
                }
                let Some(test_id) = request.test_id.as_ref() else {
                    respond("error", json!({"message":"test_id is required"}));
                    continue;
                };
                let mut r = report.lock().unwrap();
                let Some(mut test) = r.plan.tests.iter().find(|t| &t.id == test_id).cloned() else {
                    respond_unlocked(
                        &request,
                        &r.session_id,
                        "error",
                        json!({"message":"Unknown test_id"}),
                    );
                    continue;
                };
                let base = test
                    .id
                    .split("/repeat-")
                    .next()
                    .unwrap_or(&test.id)
                    .to_string();
                let count = r
                    .plan
                    .tests
                    .iter()
                    .filter(|t| t.id.starts_with(&format!("{base}/repeat-")))
                    .count()
                    + 1;
                test.id = format!("{base}/repeat-{count}");
                for equipment in &mut r.plan.equipment {
                    if test.equipment.contains(&equipment.id) {
                        equipment.required_for.push(test.id.clone());
                    }
                }
                r.plan.tests.push(test);
                r.plan.estimated_seconds = r
                    .plan
                    .tests
                    .iter()
                    .filter(|t| !r.results.iter().any(|x| x.test_id == t.id))
                    .map(|t| t.estimated_seconds)
                    .sum();
                r.state = "prepared".into();
                r.finished_at = None;
                r.refresh();
                report::write_atomic(&output, &r)?;
                respond_unlocked(
                    &request,
                    &r.session_id,
                    "repeat_prepared",
                    json!({"plan":r.plan}),
                );
            }
            "continue" | "observe" => {
                let mut current = prompt.lock().unwrap();
                let Some(p) = current.as_ref() else {
                    respond("error", json!({"message":"No action is pending"}));
                    continue;
                };
                let expected = if p.stage == "prepare" {
                    "continue"
                } else {
                    "observe"
                };
                if request.command != expected
                    || request.test_id.as_deref() != Some(&p.test_id)
                    || request.prompt_id.as_deref() != Some(&p.id)
                    || request.stage.as_deref() != Some(&p.stage)
                {
                    respond(
                        "error",
                        json!({"message":"Prompt does not match the pending action"}),
                    );
                    continue;
                }
                let verdict = request.verdict.unwrap_or(Outcome::Inconclusive);
                if !matches!(
                    verdict,
                    Outcome::Passed | Outcome::Failed | Outcome::Inconclusive | Outcome::Skipped
                ) {
                    respond("error", json!({"message":"Invalid operator verdict"}));
                    continue;
                }
                let _ = replies.send(OperatorAction::Observe(Observation {
                    verdict,
                    note: request.note.unwrap_or_default(),
                    recorded_at: report::timestamp(),
                    details: request.details.unwrap_or(json!({})),
                }));
                *current = None;
            }
            "retake" => {
                let mut current = prompt.lock().unwrap();
                let valid = current.as_ref().is_some_and(|p| {
                    p.stage == "assess"
                        && request.test_id.as_deref() == Some(&p.test_id)
                        && request.prompt_id.as_deref() == Some(&p.id)
                        && request.stage.as_deref() == Some(&p.stage)
                });
                if !valid {
                    respond(
                        "error",
                        json!({"message":"Retake does not match a pending assessment"}),
                    );
                    continue;
                }
                let test_id = current.as_ref().unwrap().test_id.clone();
                *current = None;
                respond("retaking", json!({"test_id":test_id}));
                let _ = replies.send(OperatorAction::Retake);
            }
            "cancel" => {
                cancel.store(true, Ordering::Relaxed);
                respond("cancelling", json!({}));
            }
            "export" => {
                let path = request.path.unwrap_or(output.clone());
                match report::write_atomic(&path, &report.lock().unwrap()) {
                    Ok(()) => respond("exported", json!({"path":path})),
                    Err(e) => respond("error", json!({"message":e})),
                }
            }
            "inventory" => respond("inventory", json!(report.lock().unwrap().inventory)),
            "quit" => {
                cancel.store(true, Ordering::Relaxed);
                break;
            }
            _ => respond("error", json!({"message":"Unknown command"})),
        }
    }
    if let Some(w) = worker {
        let _ = w.join();
    }
    Ok(())
}
fn respond_unlocked(request: &Request, session_id: &str, kind: &str, payload: Value) {
    emit_raw(
        json!({"protocol_version":PROTOCOL_VERSION,"session_id":session_id,"request_id":request.request_id,"type":kind,"payload":payload}),
    );
}
