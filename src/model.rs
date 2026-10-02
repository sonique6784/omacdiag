use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cpu,
    Memory,
    Storage,
    Usb,
    Camera,
    Microphone,
    Speaker,
    AudioJack,
    SdReader,
    Gpu,
    Display,
    Battery,
    Power,
    Network,
    Bluetooth,
    Keyboard,
    Trackpad,
    Thermal,
    Fan,
    Optical,
    Firewire,
    Thunderbolt,
    TouchBar,
    KeyboardBacklight,
}
impl Kind {
    pub fn key(self) -> String {
        serde_json::to_value(self).unwrap().as_str().unwrap().into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Recognition {
    Detected,
    ExpectedNotDetected,
    Unknown,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed,
    Inconclusive,
    Blocked,
    Skipped,
    Unsupported,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, clap::ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    #[default]
    Quick,
    Extended,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, clap::ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Automatic,
    Guided,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    Available,
    Unavailable,
    #[default]
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub manufacturer: String,
    pub model: String,
    pub serial: Option<String>,
    pub os: String,
    pub distribution: String,
    pub kernel: String,
    pub architecture: String,
    pub model_profile: Option<String>,
    pub expected_ports: BTreeMap<String, u32>,
    pub profile_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub recognition: Recognition,
    pub sysfs_path: Option<String>,
    pub node: Option<String>,
    pub driver: Option<String>,
    pub attributes: BTreeMap<String, String>,
    pub evidence: Vec<String>,
}
impl Device {
    pub fn detected(kind: Kind, id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            name: name.into(),
            recognition: Recognition::Detected,
            sysfs_path: None,
            node: None,
            driver: None,
            attributes: BTreeMap::new(),
            evidence: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub schema_version: u32,
    pub machine: Machine,
    pub devices: Vec<Device>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equipment {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub required_for: Vec<String>,
    pub readiness: Readiness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSpec {
    pub id: String,
    pub device_id: String,
    pub name: String,
    pub probe: String,
    pub guided: bool,
    pub required: bool,
    pub equipment: Vec<String>,
    pub permission: Option<String>,
    pub instructions: String,
    pub estimated_seconds: u64,
    pub deadline_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlan {
    pub profile: Profile,
    pub mode: Mode,
    pub tests: Vec<TestSpec>,
    pub equipment: Vec<Equipment>,
    pub estimated_seconds: u64,
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub value: Value,
    pub unit: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub verdict: Outcome,
    pub note: String,
    pub recorded_at: String,
    pub details: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_id: String,
    pub device_id: String,
    pub outcome: Outcome,
    pub reason_code: String,
    pub automatic_outcome: Option<Outcome>,
    pub automatic_reason_code: Option<String>,
    pub summary: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub measurements: BTreeMap<String, Measurement>,
    pub evidence: Vec<String>,
    pub observation: Option<Observation>,
}
impl TestResult {
    pub fn new(
        test: &TestSpec,
        outcome: Outcome,
        reason: &str,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            test_id: test.id.clone(),
            device_id: test.device_id.clone(),
            outcome,
            reason_code: reason.into(),
            automatic_outcome: None,
            automatic_reason_code: None,
            summary: summary.into(),
            started_at: crate::report::timestamp(),
            duration_ms: 0,
            measurements: BTreeMap::new(),
            evidence: vec![],
            observation: None,
        }
    }
    pub fn measure(&mut self, name: &str, value: impl Serialize, unit: &str) {
        self.measurements.insert(
            name.into(),
            Measurement {
                value: serde_json::to_value(value).unwrap_or(Value::Null),
                unit: unit.into(),
            },
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coverage {
    pub selected: usize,
    pub completed: usize,
    pub passed: usize,
    pub failed: usize,
    pub incomplete: usize,
    pub gaps: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub application_version: String,
    pub session_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub state: String,
    pub overall: String,
    pub inventory: Inventory,
    pub plan: TestPlan,
    pub results: Vec<TestResult>,
    pub coverage: Coverage,
}
