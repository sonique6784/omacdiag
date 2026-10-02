pub mod backlight;
pub mod basic;
pub mod gpu;
pub mod media;
use crate::{
    discovery,
    model::*,
    process::{self, Output},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub inventory: Inventory,
    pub baseline: Inventory,
    pub test: TestSpec,
    pub profile: Profile,
    pub observation: Option<Observation>,
    pub microphone_id: Option<String>,
}
pub fn execute(request: &WorkerRequest) -> TestResult {
    let t = &request.test;
    if !cfg!(target_os = "linux") {
        return TestResult::new(
            t,
            Outcome::Unsupported,
            "linux_required",
            "Functional diagnostics require Linux",
        );
    }
    let Some(d) = request
        .inventory
        .devices
        .iter()
        .find(|d| d.id == t.device_id)
    else {
        return TestResult::new(
            t,
            Outcome::Blocked,
            "device_missing",
            "Selected device is absent",
        );
    };
    if d.sysfs_path.is_some() && !selected_device_is_current(d) {
        return TestResult::new(
            t,
            Outcome::Blocked,
            "device_identity_changed",
            "The selected endpoint no longer matches its discovery identity; refresh inventory before testing",
        );
    }
    if d.recognition == Recognition::ExpectedNotDetected {
        return TestResult::new(
            t,
            Outcome::Inconclusive,
            "expected_not_detected",
            "Expected component was not enumerated; hardware, firmware and driver causes are not distinguished",
        );
    }
    if d.node.is_none()
        && matches!(
            t.probe.as_str(),
            "storage_health"
                | "storage_read"
                | "media_read"
                | "microphone"
                | "microphone_speech"
                | "speaker"
                | "speaker_quality"
                | "acoustic"
        )
    {
        return TestResult::new(
            t,
            Outcome::Blocked,
            "device_node_unavailable",
            "The kernel lists this device, but no usable device node is available",
        );
    }
    match t.probe.as_str() {
        "cpu" | "memory" | "storage_health" | "storage_read" | "media_read" | "battery"
        | "charging" | "power" | "network" | "network_peer" | "display" | "sensor"
        | "bluetooth" | "thunderbolt" | "usb_port" | "sd_card" => basic::execute(request, d),
        "camera" | "camera_quality" | "microphone" | "microphone_speech" | "speaker"
        | "speaker_quality" | "acoustic" | "audio_jack" => media::execute(request, d),
        "keyboard_backlight" => backlight::execute(request, d),
        "gpu" => gpu::execute(request, d),
        "manual" => manual(request, "operator_confirmed"),
        _ => TestResult::new(
            t,
            Outcome::Unsupported,
            "probe_unavailable",
            "No adapter exists for this capability",
        ),
    }
}
fn selected_device_is_current(selected: &Device) -> bool {
    let current = discovery::discover();
    current
        .devices
        .iter()
        .any(|candidate| same_device_identity(selected, candidate))
}
fn same_device_identity(selected: &Device, candidate: &Device) -> bool {
    candidate.id == selected.id
        && candidate.kind == selected.kind
        && candidate.recognition == selected.recognition
        && candidate.sysfs_path == selected.sysfs_path
        && candidate.node == selected.node
        && candidate.driver == selected.driver
        && [
            "sysfs_target",
            "node_rdev",
            "vendor",
            "device",
            "idVendor",
            "idProduct",
            "serial",
            "device/serial",
            "device/model",
            "size",
            "alsa_device",
        ]
        .iter()
        .all(|key| selected.attributes.get(*key) == candidate.attributes.get(*key))
}

pub fn manual(req: &WorkerRequest, reason: &str) -> TestResult {
    match &req.observation {
        Some(o) => {
            let mut r = TestResult::new(
                &req.test,
                o.verdict,
                reason,
                "Result is based on the technician's recorded observation",
            );
            r.observation = Some(o.clone());
            r.evidence
                .push("Operator assessment; no automatic physical fault diagnosis".into());
            r
        }
        None => TestResult::new(
            &req.test,
            Outcome::Inconclusive,
            "operator_assessment_pending",
            "A technician must exercise and assess this component after the check starts",
        ),
    }
}
pub fn run(
    program: &str,
    args: &[&str],
    input: Option<Vec<u8>>,
    seconds: u64,
    max: usize,
) -> Result<Output, String> {
    process::capture(
        process::command(program, args),
        input,
        Duration::from_secs(seconds),
        Arc::new(AtomicBool::new(false)),
        max,
    )
}
pub fn problem(req: &WorkerRequest, error: impl AsRef<str>) -> TestResult {
    let s = error.as_ref();
    let (status, reason) =
        if s.contains("Permission denied") || s.contains("Operation not permitted") {
            (Outcome::Blocked, "permission_denied")
        } else if s.contains("Device or resource busy") {
            (Outcome::Blocked, "device_busy")
        } else if s.starts_with("spawn:") {
            (Outcome::Unsupported, "dependency_unavailable")
        } else if s == "cancelled" {
            (Outcome::Cancelled, "cancelled")
        } else if s == "deadline_exceeded" {
            (Outcome::Inconclusive, "deadline_exceeded")
        } else {
            (Outcome::Inconclusive, "operation_unavailable")
        };
    TestResult::new(
        &req.test,
        status,
        reason,
        s.chars().take(2000).collect::<String>(),
    )
}

pub const FIXTURE_BYTES: usize = 64 * 1024;
pub fn fixture_byte(index: usize) -> u8 {
    ((index.wrapping_mul(73) ^ (index >> 8) ^ 0xa5) & 255) as u8
}
pub fn fixture_data() -> Vec<u8> {
    (0..FIXTURE_BYTES).map(fixture_byte).collect()
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn reused_endpoint_name_cannot_substitute_another_device() {
        let mut original = Device::detected(Kind::Storage, "storage:sda", "Fixture drive");
        original.sysfs_path = Some("/sys/class/block/sda".into());
        original.node = Some("/dev/sda".into());
        original
            .attributes
            .insert("sysfs_target".into(), "/sys/devices/pci0000:00/old".into());
        original
            .attributes
            .insert("node_rdev".into(), "2048".into());
        original
            .attributes
            .insert("device/serial".into(), "serial-a".into());
        let mut current = original.clone();
        current.attributes.insert("size".into(), "1000".into());
        assert!(!same_device_identity(&original, &current));
        original.attributes.insert("size".into(), "1000".into());
        assert!(same_device_identity(&original, &current));
        current
            .attributes
            .insert("device/serial".into(), "serial-b".into());
        assert!(!same_device_identity(&original, &current));
        current
            .attributes
            .insert("device/serial".into(), "serial-a".into());
        current
            .attributes
            .insert("sysfs_target".into(), "/sys/devices/pci0000:00/new".into());
        assert!(!same_device_identity(&original, &current));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_storage_node_blocks_a_read_without_opening_any_device() {
        use std::collections::BTreeMap;
        let device = Device::detected(Kind::Storage, "storage:sda", "Fixture drive");
        let inventory = Inventory {
            schema_version: SCHEMA_VERSION,
            machine: Machine {
                manufacturer: "Fixture".into(),
                model: "Fixture".into(),
                serial: None,
                os: "linux".into(),
                distribution: "fixture".into(),
                kernel: "fixture".into(),
                architecture: "x86_64".into(),
                model_profile: None,
                expected_ports: BTreeMap::new(),
                profile_sources: vec![],
            },
            devices: vec![device],
            warnings: vec![],
        };
        let test = TestSpec {
            id: "storage:sda/read".into(),
            device_id: "storage:sda".into(),
            name: "Sample read".into(),
            probe: "storage_read".into(),
            guided: false,
            required: true,
            equipment: vec![],
            permission: None,
            instructions: String::new(),
            estimated_seconds: 1,
            deadline_seconds: 20,
        };
        let req = WorkerRequest {
            baseline: inventory.clone(),
            inventory,
            test,
            profile: Profile::Quick,
            observation: None,
            microphone_id: None,
        };
        let result = execute(&req);
        assert_eq!(result.outcome, Outcome::Blocked);
        assert_eq!(result.reason_code, "device_node_unavailable");
    }
}
