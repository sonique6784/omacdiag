use omacdiag::{model::*, planner, profiles};
use std::collections::BTreeMap;
fn inventory(model: &str) -> Inventory {
    Inventory {
        schema_version: 1,
        machine: Machine {
            manufacturer: "Apple Inc.".into(),
            model: model.into(),
            serial: None,
            omarchy_version: None,
            board_name: None,
            board_version: None,
            os: "linux".into(),
            distribution: "fixture".into(),
            kernel: "fixture".into(),
            architecture: "x86_64".into(),
            model_profile: None,
            expected_ports: BTreeMap::new(),
            profile_sources: vec![],
        },
        pci_devices: None,
        devices: vec![],
        warnings: vec![],
    }
}
#[test]
fn published_usb_c_counts_do_not_invent_an_sd_reader() {
    let mut i = inventory("MacBookPro14,1");
    profiles::apply(&mut i);
    assert_eq!(i.machine.expected_ports.get("usb_c"), Some(&2));
    assert!(!i.devices.iter().any(|d| d.kind == Kind::SdReader));
    assert!(!i.machine.profile_sources.is_empty());
}
#[test]
fn unknown_apple_model_keeps_an_unknown_port_layout() {
    let mut i = inventory("MacBookPro99,99");
    profiles::apply(&mut i);
    assert!(i.machine.expected_ports.is_empty());
    assert!(i.machine.model_profile.unwrap().contains("baseline"));
}
#[test]
fn repeated_labels_do_not_inflate_physical_connector_coverage() {
    let mut i = inventory("MacBookPro12,1");
    profiles::apply(&mut i);
    i.devices
        .push(Device::detected(Kind::Usb, "usb:root", "Root hub"));
    let mut p = planner::build(&i, Profile::Quick, Mode::Guided, &BTreeMap::new(), None);
    p.tests.retain(|t| t.device_id == "usb:root");
    let t = p.tests[0].clone();
    let mut second = t.clone();
    second.id.push_str("/repeat-1");
    p.tests.push(second.clone());
    let mut r = Report::new(i, p);
    for (test, label) in [(t, "Left USB"), (second, "left usb")] {
        let mut result = TestResult::new(&test, Outcome::Passed, "verified", "fixture");
        result.measure("physical_port_label", label, "label");
        r.upsert(result);
    }
    assert!(
        r.coverage
            .gaps
            .iter()
            .any(|g| g.contains("1 distinct verified"))
    );
}
