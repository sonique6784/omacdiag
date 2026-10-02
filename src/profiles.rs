use crate::model::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Catalogue {
    schema_version: u32,
    profiles: Vec<ModelProfile>,
}
#[derive(Deserialize)]
struct ModelProfile {
    id: String,
    models: Vec<String>,
    expected: Vec<Kind>,
    ports: BTreeMap<String, u32>,
    sources: Vec<String>,
}
pub fn apply(inventory: &mut Inventory) {
    let apple = inventory
        .machine
        .manufacturer
        .to_lowercase()
        .contains("apple");
    let model = &inventory.machine.model;
    let family = if model.starts_with("MacBook") {
        Some("notebook")
    } else if model.starts_with("iMac") {
        Some("imac")
    } else if model.starts_with("Macmini") {
        Some("mini")
    } else if model.starts_with("MacPro") {
        Some("pro")
    } else {
        None
    };
    let Some(family) = family.filter(|_| apple) else {
        inventory.warnings.push("No Apple family profile matched. Expected hardware and physical port counts are unknown.".into());
        return;
    };
    let mut expected = vec![Kind::Cpu, Kind::Memory, Kind::Storage, Kind::Usb, Kind::Gpu];
    if family == "notebook" {
        expected.extend([
            Kind::Camera,
            Kind::Microphone,
            Kind::Speaker,
            Kind::Display,
            Kind::Battery,
            Kind::Keyboard,
            Kind::Trackpad,
        ]);
    }
    if family == "imac" {
        expected.extend([Kind::Camera, Kind::Microphone, Kind::Speaker, Kind::Display]);
    }
    let catalogue: Catalogue = serde_json::from_str(include_str!("../data/model-profiles.json"))
        .expect("Bundled profile catalogue must be valid");
    assert_eq!(catalogue.schema_version, 1);
    if let Some(profile) = catalogue
        .profiles
        .into_iter()
        .find(|p| p.models.contains(model))
    {
        inventory.machine.model_profile = Some(profile.id);
        inventory.machine.expected_ports = profile.ports;
        inventory.machine.profile_sources = profile.sources;
        expected.extend(profile.expected);
        inventory.warnings.push("Model port counts come from published specifications, not physical validation. Shared USB-C/Thunderbolt functions refer to the same connectors; extra expansion devices may change observed inventory.".into());
    } else {
        inventory.machine.model_profile = Some(format!("apple-{family}-baseline-v1"));
        inventory.warnings.push("Family baseline matched; exact release year and physical port layout are unknown. Model-specific certification remains unverified.".into());
    }
    inventory.warnings.push("A model identifier may span releases; no release year is guessed from family identity. Expected-not-detected is not proof of a hardware fault.".into());
    expected.sort();
    expected.dedup();
    for kind in expected {
        if !inventory.devices.iter().any(|d| d.kind == kind) {
            inventory.devices.push(Device {
                id: format!("expected:{}", kind.key()),
                kind,
                name: format!("Expected {}", kind.key()),
                recognition: Recognition::ExpectedNotDetected,
                sysfs_path: None,
                node: None,
                driver: None,
                attributes: BTreeMap::new(),
                evidence: vec![format!(
                    "Expected by profile {}",
                    inventory
                        .machine
                        .model_profile
                        .as_deref()
                        .unwrap_or("unknown")
                )],
            });
        }
    }
}
