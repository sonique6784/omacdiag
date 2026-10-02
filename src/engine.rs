use crate::{
    discovery,
    model::*,
    probes::{self, WorkerRequest},
    process, report,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub type EventSink = Arc<dyn Fn(&str, Value) + Send + Sync>;
pub enum OperatorAction {
    Observe(Observation),
    Retake,
}
pub type Answer = Arc<
    dyn Fn(&TestSpec, &str, Option<&TestResult>, &Inventory) -> Option<OperatorAction>
        + Send
        + Sync,
>;
pub struct RunContext {
    pub report: Arc<Mutex<Report>>,
    pub output: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub emit: EventSink,
    pub answer: Answer,
    pub microphone_id: Option<String>,
}
fn save(ctx: &RunContext) -> Result<(), String> {
    let mut r = ctx.report.lock().map_err(|_| "Report lock poisoned")?;
    r.refresh();
    report::write_atomic(&ctx.output, &r)
}
fn store(ctx: &RunContext, result: TestResult) -> Result<(), String> {
    ctx.report
        .lock()
        .map_err(|_| "Report lock poisoned")?
        .upsert(result.clone());
    save(ctx)?;
    (ctx.emit)("result", json!(result));
    Ok(())
}
pub fn run(ctx: RunContext) -> Result<(), String> {
    {
        ctx.report.lock().map_err(|_| "Report lock poisoned")?.state = "running".into();
    }
    save(&ctx)?;
    let (tests, profile, mode, inventory, equipment) = {
        let r = ctx.report.lock().unwrap();
        (
            r.plan.tests.clone(),
            r.plan.profile,
            r.plan.mode,
            r.inventory.clone(),
            r.plan.equipment.clone(),
        )
    };
    for test in &tests {
        if ctx
            .report
            .lock()
            .unwrap()
            .results
            .iter()
            .any(|r| r.test_id == test.id)
        {
            continue;
        }
        if ctx.cancel.load(Ordering::Relaxed) || process::INTERRUPTED.load(Ordering::Relaxed) {
            break;
        }
        let missing: Vec<_> = test
            .equipment
            .iter()
            .filter(|id| {
                !equipment
                    .iter()
                    .any(|e| &e.id == *id && e.readiness == Readiness::Available)
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            store(
                &ctx,
                TestResult::new(
                    test,
                    Outcome::Skipped,
                    "equipment_unavailable",
                    format!("Required equipment not confirmed: {}", missing.join(", ")),
                ),
            )?;
            continue;
        }
        if test.guided && mode == Mode::Automatic {
            store(
                &ctx,
                TestResult::new(
                    test,
                    Outcome::Skipped,
                    "operator_required",
                    "Automatic mode does not wait for physical actions or observations",
                ),
            )?;
            continue;
        }
        let mut current_test = test.clone();
        loop {
            // Each attempt needs its own pre-action baseline. A retake of an
            // insertion or charging check must not reuse the first attempt's state.
            let mut baseline = if current_test.guided {
                discovery::discover()
            } else {
                inventory.clone()
            };
            if current_test.probe == "audio_jack"
                && let Some(d) = baseline
                    .devices
                    .iter_mut()
                    .find(|d| d.id == current_test.device_id)
                && let Some(card) = d.attributes.get("card")
                && let Ok(o) = probes::run("amixer", &["-c", card, "contents"], None, 3, 128 * 1024)
            {
                d.attributes.insert(
                    "jack_controls".into(),
                    String::from_utf8_lossy(&o.stdout).into(),
                );
            }
            let observation = if needs_preparation(&current_test) {
                match (ctx.answer)(&current_test, "prepare", None, &baseline) {
                    Some(OperatorAction::Observe(o)) if o.verdict != Outcome::Skipped => Some(o),
                    Some(OperatorAction::Observe(o)) => {
                        let mut r = TestResult::new(
                            &current_test,
                            Outcome::Skipped,
                            "operator_skipped",
                            "Technician skipped this check",
                        );
                        r.observation = Some(o);
                        store(&ctx, r)?;
                        break;
                    }
                    None => {
                        ctx.cancel.store(true, Ordering::Relaxed);
                        break;
                    }
                    Some(OperatorAction::Retake) => {
                        return Err("Retake is invalid during setup".into());
                    }
                }
            } else {
                None
            };
            (ctx.emit)("test_started", json!({"test":current_test}));
            let req = WorkerRequest {
                inventory: inventory.clone(),
                baseline: baseline.clone(),
                test: current_test.clone(),
                profile,
                observation: observation.clone(),
                microphone_id: ctx.microphone_id.clone(),
            };
            let mut result = run_worker(&req, ctx.cancel.clone(), ctx.emit.clone());
            // Persist automatic evidence before waiting for the technician's final assessment.
            let assessment_needed = needs_assessment(&current_test, mode);
            let checkpoint = assessment_checkpoint(&mut result, &current_test, assessment_needed);
            store(&ctx, checkpoint)?;
            if !assessment_needed
                || ctx.cancel.load(Ordering::Relaxed)
                || result.outcome == Outcome::Cancelled
            {
                break;
            }
            let preview = if current_test.probe == "camera_quality"
                && !matches!(result.outcome, Outcome::Blocked | Outcome::Unsupported)
            {
                inventory.devices.iter().find(|d| d.id == current_test.device_id).and_then(|device| {
                    match probes::media::CameraPreview::start(device, &current_test, ctx.emit.clone()) {
                        Ok(preview) => Some(preview),
                        Err(error) => {
                            (ctx.emit)("media", json!({"test_id":current_test.id,"data":{"preview_state":"unavailable","preview_error":error}}));
                            None
                        }
                    }
                })
            } else {
                None
            };
            let action = (ctx.answer)(&current_test, "assess", Some(&result), &inventory);
            drop(preview);
            match action {
                Some(OperatorAction::Observe(answer)) => {
                    result = assess_result(result, answer, req.observation.as_ref(), &current_test);
                    store(&ctx, result)?;
                    break;
                }
                Some(OperatorAction::Retake) => {
                    if !matches!(
                        result.outcome,
                        Outcome::Failed | Outcome::Blocked | Outcome::Unsupported
                    ) {
                        result.outcome = Outcome::Inconclusive;
                        result.reason_code = "operator_requested_retake".into();
                    }
                    result.evidence.push(
                        "Technician requested another attempt before giving a verdict".into(),
                    );
                    store(&ctx, result)?;
                    current_test = append_retake(&ctx, &current_test)?;
                    let plan = ctx
                        .report
                        .lock()
                        .map_err(|_| "Report lock poisoned")?
                        .plan
                        .clone();
                    (ctx.emit)("plan", json!(plan));
                }
                None => {
                    ctx.cancel.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
    let cancelled =
        ctx.cancel.load(Ordering::Relaxed) || process::INTERRUPTED.load(Ordering::Relaxed);
    if cancelled {
        for t in &tests {
            if !ctx
                .report
                .lock()
                .unwrap()
                .results
                .iter()
                .any(|r| r.test_id == t.id)
            {
                store(
                    &ctx,
                    TestResult::new(
                        t,
                        Outcome::Cancelled,
                        "cancelled",
                        "Session cancelled before this test completed",
                    ),
                )?;
            }
        }
    }
    {
        let mut r = ctx.report.lock().unwrap();
        r.state = if cancelled { "cancelled" } else { "completed" }.into();
        r.finished_at = Some(report::timestamp());
    }
    save(&ctx)?;
    let r = ctx.report.lock().unwrap().clone();
    (ctx.emit)("finished", json!({"report":r,"path":ctx.output}));
    Ok(())
}

fn append_retake(ctx: &RunContext, previous: &TestSpec) -> Result<TestSpec, String> {
    let mut report = ctx.report.lock().map_err(|_| "Report lock poisoned")?;
    let base = previous.id.split("/retake-").next().unwrap_or(&previous.id);
    let next_number = report
        .plan
        .tests
        .iter()
        .filter_map(|test| {
            test.id
                .strip_prefix(&format!("{base}/retake-"))?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0)
        + 1;
    let mut next = previous.clone();
    next.id = format!("{base}/retake-{next_number}");
    next.required = true;
    let prior = report
        .plan
        .tests
        .iter_mut()
        .find(|test| test.id == previous.id)
        .ok_or("Previous retake attempt is missing from the plan")?;
    prior.required = false;
    report.plan.estimated_seconds += next.estimated_seconds;
    report.plan.tests.push(next.clone());
    report.refresh();
    report::write_atomic(&ctx.output, &report)?;
    Ok(next)
}

fn needs_preparation(test: &TestSpec) -> bool {
    test.guided
        && matches!(
            test.probe.as_str(),
            "usb_port"
                | "sd_card"
                | "charging"
                | "network_peer"
                | "bluetooth"
                | "audio_jack"
                | "media_read"
                | "thunderbolt"
                | "microphone_speech"
                | "speaker_quality"
        )
}

fn needs_assessment(test: &TestSpec, mode: Mode) -> bool {
    mode == Mode::Guided && test.guided
}

fn run_worker(req: &WorkerRequest, cancel: Arc<AtomicBool>, emit: EventSink) -> TestResult {
    let start = Instant::now();
    let mut command = match std::env::current_exe() {
        Ok(exe) => Command::new(exe),
        Err(e) => return probes::problem(req, e.to_string()),
    };
    command
        .arg("_worker")
        .env("OMACDIAG_WORKER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    process::isolated(&mut command);
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return probes::problem(req, format!("spawn: {e}")),
    };
    let mut input = child.stdin.take().unwrap();
    if serde_json::to_writer(&mut input, req)
        .and_then(|_| input.flush().map_err(serde_json::Error::io))
        .is_err()
    {
        process::terminate(&mut child);
        return probes::problem(req, "Worker input failed");
    }
    drop(input);
    let reader = child.stdout.take().unwrap();
    let errors = child.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let read_thread = thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = Vec::new();
            match Read::by_ref(&mut reader)
                .take(256 * 1024 + 1)
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > 256 * 1024 => break,
                Ok(_) => {
                    if let Ok(v) = serde_json::from_slice::<Value>(&line) {
                        let _ = tx.send(v);
                    }
                }
            }
        }
    });
    let error_thread = thread::spawn(move || {
        let mut reader = BufReader::new(errors);
        let mut text = String::new();
        let _ = reader.by_ref().take(16384).read_to_string(&mut text);
        text
    });
    let mut result = None;
    let mut failure = None;
    loop {
        while let Ok(event) = rx.try_recv() {
            if event.get("type").and_then(Value::as_str) == Some("worker_result") {
                result = event
                    .get("payload")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok());
            } else if event.get("type").and_then(Value::as_str) == Some("media") {
                emit(
                    "media",
                    event.get("payload").cloned().unwrap_or(Value::Null),
                );
            }
        }
        if cancel.load(Ordering::Relaxed) || process::INTERRUPTED.load(Ordering::Relaxed) {
            failure = Some((Outcome::Cancelled, "cancelled", "Test cancelled"));
            process::terminate(&mut child);
            break;
        }
        if start.elapsed() > Duration::from_secs(req.test.deadline_seconds) {
            failure = Some((
                Outcome::Inconclusive,
                "deadline_exceeded",
                "Worker exceeded its deadline",
            ));
            process::terminate(&mut child);
            break;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    failure = Some((
                        Outcome::Inconclusive,
                        "worker_failed",
                        "Diagnostic worker exited abnormally",
                    ));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(_) => {
                process::terminate(&mut child);
                failure = Some((Outcome::Inconclusive, "worker_failed", "Worker wait failed"));
                break;
            }
        }
    }
    // Close inherited pipes by terminating any worker descendants before joining readers.
    process::terminate(&mut child);
    let _ = read_thread.join();
    let error = error_thread.join().unwrap_or_default();
    while let Ok(event) = rx.try_recv() {
        if event.get("type").and_then(Value::as_str) == Some("worker_result") {
            result = event
                .get("payload")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok());
        } else if event.get("type").and_then(Value::as_str) == Some("media") {
            emit(
                "media",
                event.get("payload").cloned().unwrap_or(Value::Null),
            );
        }
    }
    let mut result = if let Some((outcome, reason, message)) = failure {
        TestResult::new(&req.test, outcome, reason, message)
    } else {
        result.unwrap_or_else(|| {
            TestResult::new(
                &req.test,
                Outcome::Inconclusive,
                "worker_result_missing",
                "No valid worker result was returned",
            )
        })
    };
    result.duration_ms = start.elapsed().as_millis() as u64;
    if !error.is_empty() {
        result.evidence.push(error);
    }
    result
}

pub fn readiness(values: &[String]) -> BTreeMap<String, Readiness> {
    values
        .iter()
        .map(|v| (v.clone(), Readiness::Available))
        .collect()
}
fn assessment_checkpoint(
    result: &mut TestResult,
    test: &TestSpec,
    assessment_needed: bool,
) -> TestResult {
    if test.probe != "manual" {
        result.automatic_outcome = Some(result.outcome);
        result.automatic_reason_code = Some(result.reason_code.clone());
    }
    let mut checkpoint = result.clone();
    if assessment_needed && checkpoint.outcome == Outcome::Passed {
        checkpoint.outcome = Outcome::Inconclusive;
        checkpoint.reason_code = "awaiting_operator_assessment".into();
    }
    checkpoint
}
fn assess_result(
    mut result: TestResult,
    mut observation: Observation,
    prepared: Option<&Observation>,
    test: &TestSpec,
) -> TestResult {
    let automatic = result.outcome;
    let manual_only = test.probe == "manual";
    result.outcome = assessed_outcome(manual_only, automatic, observation.verdict);
    if result.outcome == Outcome::Failed && observation.verdict == Outcome::Failed {
        result.reason_code = "operator_observed_failure".into();
    } else if result.outcome == Outcome::Skipped {
        result.reason_code = "operator_skipped".into();
    } else if manual_only
        && !matches!(
            result.outcome,
            Outcome::Blocked | Outcome::Unsupported | Outcome::Cancelled
        )
    {
        result.reason_code = "operator_assessment".into();
    } else if automatic == Outcome::Passed && result.outcome == Outcome::Inconclusive {
        result.reason_code = "operator_inconclusive".into();
    }
    if let Some(prepared) = prepared
        && let (Some(a), Some(b)) = (
            prepared.details.as_object(),
            observation.details.as_object_mut(),
        )
    {
        for (key, value) in a {
            b.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    result.observation = Some(observation);
    result
}
/// Combine independent evidence without allowing an observation to hide a failure.
pub fn assessed_outcome(manual_only: bool, automatic: Outcome, operator: Outcome) -> Outcome {
    if automatic == Outcome::Failed || operator == Outcome::Failed {
        return Outcome::Failed;
    }
    if matches!(
        automatic,
        Outcome::Blocked | Outcome::Unsupported | Outcome::Cancelled
    ) {
        return automatic;
    }
    match operator {
        Outcome::Passed if automatic == Outcome::Passed || manual_only => Outcome::Passed,
        Outcome::Skipped => Outcome::Skipped,
        _ => Outcome::Inconclusive,
    }
}
pub fn default_output(report: &Report) -> PathBuf {
    Path::new("reports").join(format!("{}.json", report.session_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test(probe: &str) -> TestSpec {
        TestSpec {
            id: "test".into(),
            device_id: "device".into(),
            name: "Fixture".into(),
            probe: probe.into(),
            guided: true,
            required: true,
            equipment: vec![],
            permission: None,
            instructions: String::new(),
            estimated_seconds: 1,
            deadline_seconds: 20,
        }
    }
    fn observation(verdict: Outcome) -> Observation {
        Observation {
            verdict,
            note: String::new(),
            recorded_at: report::timestamp(),
            details: json!({}),
        }
    }

    #[test]
    fn assessment_preserves_independent_automatic_evidence() {
        let cases = [
            (
                Outcome::Failed,
                Outcome::Skipped,
                Outcome::Failed,
                "automatic",
            ),
            (
                Outcome::Failed,
                Outcome::Passed,
                Outcome::Failed,
                "automatic",
            ),
            (
                Outcome::Failed,
                Outcome::Inconclusive,
                Outcome::Failed,
                "automatic",
            ),
            (
                Outcome::Blocked,
                Outcome::Passed,
                Outcome::Blocked,
                "automatic",
            ),
            (
                Outcome::Unsupported,
                Outcome::Passed,
                Outcome::Unsupported,
                "automatic",
            ),
            (
                Outcome::Cancelled,
                Outcome::Passed,
                Outcome::Cancelled,
                "automatic",
            ),
            (
                Outcome::Passed,
                Outcome::Failed,
                Outcome::Failed,
                "operator_observed_failure",
            ),
            (
                Outcome::Passed,
                Outcome::Inconclusive,
                Outcome::Inconclusive,
                "operator_inconclusive",
            ),
            (
                Outcome::Passed,
                Outcome::Skipped,
                Outcome::Skipped,
                "operator_skipped",
            ),
            (
                Outcome::Passed,
                Outcome::Passed,
                Outcome::Passed,
                "automatic",
            ),
        ];
        let test = test("camera");
        for (automatic, operator, combined, reason) in cases {
            let mut result = TestResult::new(&test, automatic, "automatic", "Fixture");
            let checkpoint = assessment_checkpoint(&mut result, &test, true);
            if automatic == Outcome::Passed {
                assert_eq!(checkpoint.outcome, Outcome::Inconclusive);
                assert_eq!(checkpoint.reason_code, "awaiting_operator_assessment");
            }
            assert_eq!(checkpoint.automatic_outcome, Some(automatic));
            assert_eq!(
                checkpoint.automatic_reason_code.as_deref(),
                Some("automatic")
            );
            let result = assess_result(result, observation(operator), None, &test);
            assert_eq!(result.outcome, combined, "{automatic:?} + {operator:?}");
            assert_eq!(result.reason_code, reason);
            assert_eq!(result.automatic_outcome, Some(automatic));
            assert_eq!(result.automatic_reason_code.as_deref(), Some("automatic"));
        }
    }

    #[test]
    fn manual_assessment_has_no_automatic_evidence_and_keeps_preparation_details() {
        let test = test("manual");
        let mut result =
            TestResult::new(&test, Outcome::Inconclusive, "operator_required", "Fixture");
        let checkpoint = assessment_checkpoint(&mut result, &test, true);
        assert_eq!(checkpoint.automatic_outcome, None);
        assert_eq!(checkpoint.automatic_reason_code, None);
        let prepared = Observation {
            details: json!({"port_label":"left", "shared":"preparation"}),
            ..observation(Outcome::Inconclusive)
        };
        let final_observation = Observation {
            details: json!({"shared":"assessment"}),
            ..observation(Outcome::Passed)
        };
        let result = assess_result(result, final_observation, Some(&prepared), &test);
        assert_eq!(result.outcome, Outcome::Passed);
        assert_eq!(result.reason_code, "operator_assessment");
        assert_eq!(result.automatic_outcome, None);
        assert_eq!(
            result.observation.unwrap().details,
            json!({"port_label":"left", "shared":"assessment"})
        );
    }

    #[test]
    fn only_checks_with_required_setup_get_a_pre_run_prompt() {
        assert!(!needs_preparation(&test("keyboard_backlight")));
        assert!(!needs_preparation(&test("manual")));
        assert!(!needs_preparation(&test("camera_quality")));
        assert!(needs_preparation(&test("usb_port")));
        assert!(needs_preparation(&test("charging")));
        assert!(needs_preparation(&test("microphone_speech")));
    }

    #[test]
    fn guided_mode_keeps_automatic_checks_automatic() {
        let mut cpu = test("cpu");
        cpu.guided = false;
        let mut result = TestResult::new(&cpu, Outcome::Passed, "cpu_ok", "CPU calculation passed");
        let assessment_needed = needs_assessment(&cpu, Mode::Guided);
        let checkpoint = assessment_checkpoint(&mut result, &cpu, assessment_needed);
        assert_eq!(checkpoint.outcome, Outcome::Passed);
        assert_eq!(checkpoint.automatic_outcome, Some(Outcome::Passed));
        assert_eq!(checkpoint.automatic_reason_code.as_deref(), Some("cpu_ok"));
        assert!(!needs_preparation(&cpu));
        let mut battery = test("battery");
        battery.guided = false;
        assert!(!needs_assessment(&battery, Mode::Guided));
        assert!(needs_assessment(&test("manual"), Mode::Guided));
        assert!(!needs_assessment(&test("manual"), Mode::Automatic));
    }

    #[test]
    fn retake_keeps_history_but_requires_only_the_latest_attempt() {
        let original = test("keyboard_backlight");
        let inventory = Inventory {
            schema_version: 1,
            machine: Machine {
                manufacturer: "Apple".into(),
                model: "fixture".into(),
                serial: None,
                os: "linux".into(),
                distribution: "fixture".into(),
                kernel: "fixture".into(),
                architecture: "x86_64".into(),
                model_profile: None,
                expected_ports: BTreeMap::new(),
                profile_sources: vec![],
            },
            devices: vec![],
            warnings: vec![],
        };
        let plan = TestPlan {
            profile: Profile::Quick,
            mode: Mode::Guided,
            tests: vec![original.clone()],
            equipment: vec![],
            estimated_seconds: 1,
            notices: vec![],
        };
        let mut report = Report::new(inventory, plan);
        report.upsert(TestResult::new(
            &original,
            Outcome::Inconclusive,
            "operator_requested_retake",
            "First attempt",
        ));
        let output =
            std::env::temp_dir().join(format!("omacdiag-retake-{}.json", report.session_id));
        let ctx = RunContext {
            report: Arc::new(Mutex::new(report)),
            output: output.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            emit: Arc::new(|_, _| {}),
            answer: Arc::new(|_, _, _, _| None),
            microphone_id: None,
        };
        let next = append_retake(&ctx, &original).unwrap();
        assert_eq!(next.id, "test/retake-1");
        {
            let mut report = ctx.report.lock().unwrap();
            assert!(!report.plan.tests[0].required);
            assert!(report.plan.tests[1].required);
            report.upsert(TestResult::new(
                &next,
                Outcome::Passed,
                "automatic",
                "Second attempt",
            ));
            report.state = "completed".into();
            report.refresh();
            assert_eq!(report.overall, "passed");
            assert_eq!(report.coverage.incomplete, 0);
            assert_eq!(report.results.len(), 2);
        }
        let saved: Report = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(saved.plan.tests.len(), 2);
        let _ = std::fs::remove_file(output);
    }
}
