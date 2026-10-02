use omacdiag::{
    discovery::{self, LinuxPaths},
    engine::{self, RunContext},
    model::*,
    planner,
    probes::{self, media},
    process, report,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omacdiag-test-{}-{}",
            report::session_id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn put(&self, path: &str, value: &str) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, value).unwrap();
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn inventory(devices: Vec<Device>) -> Inventory {
    Inventory {
        schema_version: 1,
        machine: Machine {
            manufacturer: "Apple Inc.".into(),
            model: "MacBookPro12,1".into(),
            serial: None,
            os: "linux".into(),
            distribution: "fixture".into(),
            kernel: "fixture".into(),
            architecture: "x86_64".into(),
            model_profile: None,
            expected_ports: BTreeMap::new(),
            profile_sources: vec![],
        },
        devices,
        warnings: vec![],
    }
}
fn output_device() -> Device {
    let mut d = Device::detected(Kind::Speaker, "speaker:pcmC0D0p", "Internal output");
    d.attributes
        .insert("alsa_device".into(), "plughw:CARD=PCH,DEV=0".into());
    d
}

#[test]
fn keyboard_backlight_requires_a_guided_visual_confirmation() {
    let backlight = Device::detected(
        Kind::KeyboardBacklight,
        "keyboard_backlight:smc::kbd_backlight",
        "Keyboard backlight",
    );
    let plan = planner::build(
        &inventory(vec![backlight]),
        Profile::Quick,
        Mode::Guided,
        &BTreeMap::new(),
        None,
    );
    let check = plan
        .tests
        .iter()
        .find(|test| test.device_id == "keyboard_backlight:smc::kbd_backlight")
        .unwrap();
    assert_eq!(check.probe, "keyboard_backlight");
    assert!(check.guided);
    assert!(check.instructions.contains("visibly changed"));
    assert!(check.permission.as_deref().unwrap().contains("UPower"));
}

#[test]
fn pci_camera_is_detected_without_a_usb_or_capture_endpoint() {
    let s = Scratch::new();
    s.put("sys/class/dmi/id/sys_vendor", "Apple Inc.");
    s.put("sys/class/dmi/id/product_name", "MacBookPro12,1");
    s.put("sys/bus/pci/devices/0000:02:00.0/class", "0x048000");
    s.put("sys/bus/pci/devices/0000:02:00.0/vendor", "0x14e4");
    s.put("sys/bus/pci/devices/0000:02:00.0/device", "0x1570");
    s.put("etc/os-release", "PRETTY_NAME=\"Fixture Linux\"");
    let i = discovery::discover_at(&LinuxPaths {
        sys: s.0.join("sys"),
        proc: s.0.join("proc"),
        etc: s.0.join("etc"),
        dev: s.0.join("dev"),
    });
    let cameras: Vec<_> = i
        .devices
        .iter()
        .filter(|d| d.kind == Kind::Camera)
        .collect();
    assert_eq!(cameras.len(), 1);
    assert_eq!(cameras[0].recognition, Recognition::Detected);
    assert!(cameras[0].node.is_none());
    assert!(
        i.devices
            .iter()
            .any(|d| d.kind == Kind::Battery && d.recognition == Recognition::ExpectedNotDetected)
    );
}
#[test]
fn enumerated_video_device_without_a_node_is_not_reported_as_usable() {
    let s = Scratch::new();
    s.put("sys/class/dmi/id/sys_vendor", "Apple Inc.");
    s.put("sys/class/dmi/id/product_name", "MacBookAir5,1");
    s.put("sys/class/video4linux/video0/name", "Fixture camera");
    s.put("dev/video0", "not a character device");
    let i = discovery::discover_at(&LinuxPaths {
        sys: s.0.join("sys"),
        proc: s.0.join("proc"),
        etc: s.0.join("etc"),
        dev: s.0.join("dev"),
    });
    let camera = i.devices.iter().find(|d| d.id == "camera:video0").unwrap();
    assert!(camera.node.is_none());
    assert!(
        camera
            .evidence
            .iter()
            .any(|item| item.contains("No usable device node"))
    );
}
#[test]
fn unknown_models_do_not_invent_expected_hardware() {
    let s = Scratch::new();
    s.put("sys/class/dmi/id/sys_vendor", "Other");
    s.put("sys/class/dmi/id/product_name", "Mystery");
    let i = discovery::discover_at(&LinuxPaths {
        sys: s.0.join("sys"),
        proc: s.0.join("proc"),
        etc: s.0.join("etc"),
        dev: s.0.join("dev"),
    });
    assert!(i.machine.model_profile.is_none());
    assert!(i.devices.is_empty());
}
#[test]
fn equipment_list_tracks_selected_tests_only() {
    let usb = Device::detected(Kind::Usb, "usb:pci", "USB controller");
    let display = Device::detected(Kind::Display, "display:HDMI-A-1", "HDMI");
    let i = inventory(vec![usb, display]);
    let all = planner::build(&i, Profile::Quick, Mode::Guided, &BTreeMap::new(), None);
    assert!(
        all.equipment
            .iter()
            .any(|e| e.id == "usb_media" && e.readiness == Readiness::Uncertain)
    );
    assert!(all.equipment.iter().any(|e| e.id == "external_display"));
    let selected = std::collections::BTreeSet::from(["usb:pci/port".into()]);
    let plan = planner::build(
        &i,
        Profile::Quick,
        Mode::Guided,
        &engine::readiness(&["usb_media".into()]),
        Some(&selected),
    );
    assert_eq!(plan.equipment.len(), 1);
    assert_eq!(plan.equipment[0].readiness, Readiness::Available);
    assert_eq!(plan.tests.len(), 1);
}
#[test]
fn external_mic_is_required_only_when_no_physical_input_is_detected() {
    let output = output_device();
    let without = planner::build(
        &inventory(vec![output.clone()]),
        Profile::Quick,
        Mode::Automatic,
        &BTreeMap::new(),
        None,
    );
    assert!(
        without
            .equipment
            .iter()
            .any(|e| e.id == "external_microphone")
    );
    let with = planner::build(
        &inventory(vec![
            output,
            Device::detected(Kind::Microphone, "mic:0", "Mic"),
        ]),
        Profile::Quick,
        Mode::Automatic,
        &BTreeMap::new(),
        None,
    );
    assert!(!with.equipment.iter().any(|e| e.id == "external_microphone"));
}
#[test]
fn hdmi_pcm_is_planned_as_display_audio_not_a_built_in_speaker() {
    let s = Scratch::new();
    s.put("sys/class/dmi/id/sys_vendor", "Apple Inc.");
    s.put("sys/class/dmi/id/product_name", "MacBookAir5,1");
    s.put("sys/class/sound/card0/id", "PCH");
    s.put("sys/class/sound/pcmC0D0p/uevent", "");
    s.put("sys/class/sound/pcmC0D3p/uevent", "");
    s.put("proc/asound/card0/pcm0p/info", "name: CS4206 Analog\n");
    s.put("proc/asound/card0/pcm3p/info", "name: HDMI 0\n");
    let i = discovery::discover_at(&LinuxPaths {
        sys: s.0.join("sys"),
        proc: s.0.join("proc"),
        etc: s.0.join("etc"),
        dev: s.0.join("dev"),
    });
    let analog = i
        .devices
        .iter()
        .find(|d| d.id == "speaker:pcmC0D0p")
        .unwrap();
    let hdmi = i
        .devices
        .iter()
        .find(|d| d.id == "speaker:pcmC0D3p")
        .unwrap();
    assert_eq!(
        analog.attributes.get("playback_route").map(String::as_str),
        Some("analog")
    );
    assert_eq!(
        hdmi.attributes.get("playback_route").map(String::as_str),
        Some("display")
    );
    assert!(hdmi.name.contains("HDMI"));
    let plan = planner::build(&i, Profile::Quick, Mode::Guided, &BTreeMap::new(), None);
    assert!(
        plan.tests
            .iter()
            .any(|t| t.id == "speaker:pcmC0D0p/acoustic")
    );
    assert!(
        !plan
            .tests
            .iter()
            .any(|t| t.id == "speaker:pcmC0D3p/acoustic")
    );
    let hdmi_test = plan
        .tests
        .iter()
        .find(|t| t.id == "speaker:pcmC0D3p/playback")
        .unwrap();
    assert!(hdmi_test.equipment.contains(&"external_display".into()));
    assert!(hdmi_test.name.contains("Digital display audio"));
}
#[test]
fn incomplete_outcomes_and_empty_selection_never_pass() {
    let i = inventory(vec![Device::detected(Kind::Cpu, "cpu:system", "CPU")]);
    let plan = planner::build(&i, Profile::Quick, Mode::Automatic, &BTreeMap::new(), None);
    let test = plan.tests[0].clone();
    for outcome in [
        Outcome::Skipped,
        Outcome::Unsupported,
        Outcome::Blocked,
        Outcome::Inconclusive,
        Outcome::Cancelled,
    ] {
        let mut r = Report::new(i.clone(), plan.clone());
        r.state = "completed".into();
        r.upsert(TestResult::new(&test, outcome, "fixture", "fixture"));
        assert_eq!(r.overall, "incomplete");
        assert_eq!(r.coverage.incomplete, 1);
    }
    let mut r = Report::new(i.clone(), plan);
    r.state = "completed".into();
    r.upsert(TestResult::new(
        &test,
        Outcome::Failed,
        "mismatch",
        "mismatch",
    ));
    assert_eq!(r.overall, "failed");
    r.upsert(TestResult::new(
        &test,
        Outcome::Passed,
        "verified",
        "verified",
    ));
    assert_eq!(r.overall, "passed");
    let mut empty = Report::new(
        i.clone(),
        planner::build(
            &i,
            Profile::Quick,
            Mode::Automatic,
            &BTreeMap::new(),
            Some(&Default::default()),
        ),
    );
    empty.state = "completed".into();
    empty.refresh();
    assert_eq!(empty.overall, "incomplete");
}
#[test]
fn report_checkpoints_replace_atomically_and_preserve_observations() {
    let s = Scratch::new();
    let i = inventory(vec![Device::detected(Kind::Cpu, "cpu:system", "CPU")]);
    let p = planner::build(&i, Profile::Quick, Mode::Automatic, &BTreeMap::new(), None);
    let mut r = Report::new(i, p);
    let path = s.0.join("report.json");
    report::write_atomic(&path, &r).unwrap();
    let mut result = TestResult::new(
        &r.plan.tests[0],
        Outcome::Inconclusive,
        "awaiting_operator_assessment",
        "Evidence pending assessment",
    );
    result.automatic_outcome = Some(Outcome::Passed);
    result.automatic_reason_code = Some("automatic_sample_passed".into());
    result.observation = Some(Observation {
        verdict: Outcome::Inconclusive,
        note: "Uncertain".into(),
        recorded_at: report::timestamp(),
        details: json!({"port_label":"left USB"}),
    });
    r.upsert(result);
    report::write_atomic(&path, &r).unwrap();
    let saved: Report = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.results.len(), 1);
    assert_eq!(saved.overall, "incomplete");
    assert_eq!(saved.results[0].automatic_outcome, Some(Outcome::Passed));
    assert_eq!(
        saved.results[0].automatic_reason_code.as_deref(),
        Some("automatic_sample_passed")
    );
    assert_eq!(fs::read_dir(&s.0).unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn missing_equipment_is_skipped_without_asking_an_operator() {
    let s = Scratch::new();
    let i = inventory(vec![Device::detected(Kind::Usb, "usb:controller", "USB")]);
    let p = planner::build(&i, Profile::Quick, Mode::Guided, &BTreeMap::new(), None);
    let r = Arc::new(Mutex::new(Report::new(i, p)));
    engine::run(RunContext {
        report: r.clone(),
        output: s.0.join("report.json"),
        cancel: Arc::new(AtomicBool::new(false)),
        emit: Arc::new(|_, _| {}),
        answer: Arc::new(|_, _, _, _| panic!("Missing equipment must not prompt")),
        microphone_id: None,
    })
    .unwrap();
    assert_eq!(r.lock().unwrap().results[0].outcome, Outcome::Skipped);
    assert_eq!(r.lock().unwrap().overall, "incomplete");
}
#[test]
fn automatic_mode_never_waits_for_physical_actions() {
    let s = Scratch::new();
    let i = inventory(vec![Device::detected(Kind::Usb, "usb:controller", "USB")]);
    let p = planner::build(
        &i,
        Profile::Quick,
        Mode::Automatic,
        &engine::readiness(&["usb_media".into()]),
        None,
    );
    let r = Arc::new(Mutex::new(Report::new(i, p)));
    engine::run(RunContext {
        report: r.clone(),
        output: s.0.join("report.json"),
        cancel: Arc::new(AtomicBool::new(false)),
        emit: Arc::new(|_, _| {}),
        answer: Arc::new(|_, _, _, _| panic!("Automatic mode cannot prompt")),
        microphone_id: None,
    })
    .unwrap();
    assert_eq!(
        r.lock().unwrap().results[0].reason_code,
        "operator_required"
    );
}
#[test]
fn cancellation_marks_pending_checks_and_writes_a_partial_report() {
    let s = Scratch::new();
    let i = inventory(vec![output_device()]);
    let p = planner::build(&i, Profile::Quick, Mode::Automatic, &BTreeMap::new(), None);
    let r = Arc::new(Mutex::new(Report::new(i, p)));
    engine::run(RunContext {
        report: r.clone(),
        output: s.0.join("report.json"),
        cancel: Arc::new(AtomicBool::new(true)),
        emit: Arc::new(|_, _| {}),
        answer: Arc::new(|_, _, _, _| None),
        microphone_id: None,
    })
    .unwrap();
    let r = r.lock().unwrap();
    assert_eq!(r.state, "cancelled");
    assert_eq!(r.results.len(), r.plan.tests.len());
    assert!(r.results.iter().all(|r| r.outcome == Outcome::Cancelled));
    assert_eq!(r.overall, "incomplete");
}
#[test]
fn matched_acoustic_signal_is_distinguished_from_silence() {
    let reference: Vec<_> = (0..120)
        .map(|i| ((i * 73 % 131) as f64 - 65.0) / 65.0)
        .collect();
    let mut recorded = vec![0.0; 30];
    recorded.extend(reference.iter().map(|v| v * 0.2));
    recorded.extend(vec![0.0; 60]);
    let (score, lag) = media::correlation(&reference, &recorded);
    assert!(score > 0.99);
    assert_eq!(lag, 30);
    assert_eq!(media::correlation(&reference, &vec![0.0; 210]).0, 0.0);
}
#[test]
fn clipped_and_silent_microphone_levels_are_explicit() {
    let (db, peak, clip) = media::levels(&vec![0.0; 128]);
    assert_eq!(db, -120.0);
    assert_eq!(peak, 0.0);
    assert_eq!(clip, 0.0);
    let (_, _, clip) = media::levels(&vec![1.0; 128]);
    assert_eq!(clip, 1.0);
    assert_eq!(media::base64(b"hello"), "aGVsbG8=");
}
#[test]
fn fixture_has_exact_deterministic_reference_content() {
    let data = probes::fixture_data();
    assert_eq!(data.len(), 65536);
    assert_eq!(data[0], 0xa5);
    assert_eq!(data[43210], probes::fixture_byte(43210));
}

#[test]
fn unknown_test_and_equipment_ids_are_rejected() {
    let i = inventory(vec![Device::detected(Kind::Cpu, "cpu:system", "CPU")]);
    assert!(
        planner::validate(
            &i,
            Profile::Quick,
            Mode::Automatic,
            &engine::readiness(&["typo".into()]),
            None
        )
        .is_err()
    );
    assert!(
        planner::validate(
            &i,
            Profile::Quick,
            Mode::Automatic,
            &BTreeMap::new(),
            Some(&std::collections::BTreeSet::from(["missing-test".into()]))
        )
        .is_err()
    );
}
#[test]
fn subprocess_deadline_terminates_a_stuck_process() {
    let error = process::capture(
        process::command("/bin/sh", &["-c", "sleep 20"]),
        None,
        Duration::from_millis(100),
        Arc::new(AtomicBool::new(false)),
        1024,
    )
    .err()
    .unwrap();
    assert_eq!(error, "deadline_exceeded");
}
#[test]
fn subprocess_cancellation_terminates_a_nested_process_promptly() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let timer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        signal.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let start = Instant::now();
    let result = process::capture(
        process::command("/bin/sh", &["-c", "sleep 20 & wait"]),
        None,
        Duration::from_secs(10),
        cancelled,
        1024,
    );
    timer.join().unwrap();
    assert_eq!(result.err().as_deref(), Some("cancelled"));
    assert!(start.elapsed() < Duration::from_secs(3));
}
#[test]
fn subprocess_output_is_bounded_without_pipe_deadlock() {
    let out = process::capture(
        process::command("/bin/sh", &["-c", "yes x | head -c 65536"]),
        None,
        Duration::from_secs(2),
        Arc::new(AtomicBool::new(false)),
        64,
    )
    .unwrap();
    assert_eq!(out.stdout.len(), 64);
    assert_eq!(out.code, Some(0));
}
