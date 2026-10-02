use omacdiag::{model::*, planner};
use serde_json::json;
use std::collections::BTreeMap;

// Synthetic contract coverage only. Never a hardware diagnostic result.
fn main() {
    let kinds = [
        Kind::Cpu,
        Kind::Memory,
        Kind::Storage,
        Kind::Usb,
        Kind::Camera,
        Kind::Microphone,
        Kind::Speaker,
        Kind::AudioJack,
        Kind::SdReader,
        Kind::Gpu,
        Kind::Display,
        Kind::Battery,
        Kind::Power,
        Kind::Network,
        Kind::Bluetooth,
        Kind::Keyboard,
        Kind::Trackpad,
        Kind::Thermal,
        Kind::Fan,
        Kind::Optical,
        Kind::Firewire,
        Kind::Thunderbolt,
        Kind::TouchBar,
        Kind::KeyboardBacklight,
    ];
    let inventory = Inventory {
        schema_version: 1,
        machine: Machine {
            manufacturer: "SYNTHETIC FIXTURE".into(),
            model: "contract-fixture".into(),
            serial: None,
            os: "fixture".into(),
            distribution: "fixture".into(),
            kernel: "fixture".into(),
            architecture: "x86_64".into(),
            model_profile: None,
            expected_ports: BTreeMap::new(),
            profile_sources: vec![],
        },
        devices: kinds
            .into_iter()
            .map(|k| Device::detected(k, format!("fixture:{}", k.key()), "Synthetic device"))
            .collect(),
        warnings: vec!["Synthetic schema coverage; not evidence from a real machine".into()],
    };
    let plan = planner::build(
        &inventory,
        Profile::Extended,
        Mode::Guided,
        &BTreeMap::new(),
        None,
    );
    let mut report = Report::new(inventory, plan);
    let outcomes = [
        Outcome::Passed,
        Outcome::Failed,
        Outcome::Inconclusive,
        Outcome::Blocked,
        Outcome::Skipped,
        Outcome::Unsupported,
        Outcome::Cancelled,
    ];
    for (index, test) in report.plan.tests.clone().iter().enumerate() {
        let mut result = TestResult::new(
            test,
            outcomes[index % outcomes.len()],
            "synthetic",
            "Synthetic schema fixture only",
        );
        if test.probe != "manual" && index % 2 == 0 {
            result.automatic_outcome = Some(result.outcome);
            result.automatic_reason_code = Some("synthetic_automatic".into());
        }
        result.measure("number", 42.25, "units");
        result.measure("boolean", true, "boolean");
        result.measure("object", json!({"items":[1,"text",null]}), "structured");
        if test.guided {
            result.observation = Some(Observation {
                verdict: Outcome::Inconclusive,
                note: "Synthetic operator".into(),
                recorded_at: "0".into(),
                details: json!({"port_label":"synthetic left"}),
            });
        }
        report.upsert(result);
    }
    report.state = "completed".into();
    report.finished_at = Some("0".into());
    report.refresh();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
