use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

pub fn preparation() -> Vec<Equipment> {
    [
        ("usb_media", "Known working USB media", "Prepare omacdiag-fixture.bin on the media with `omacdiag fixture`. Use a direct connection where possible; label every physical port you check."),
        ("sd_card", "Known working SD card", "Prepare omacdiag-fixture.bin with `omacdiag fixture`. Use a compatible card; avoid adapters that bypass the built-in reader."),
        ("headset", "Known working headset", "Use headphones and a microphone connector compatible with this Mac. A headphones-only jack must not be reported as a microphone failure."),
        ("external_display", "Monitor, cables and adapters", "Power the monitor and prepare cables for the actual connector. Use an audio-capable display for digital audio checks. USB-C, HDMI, Mini DisplayPort and Thunderbolt are different capabilities."),
        ("charger", "Compatible charger or power cable", "Use a known working supply with the correct connector and adequate wattage."),
        ("network_peer", "Designated network and test peer", "Prepare Wi-Fi credentials or an Ethernet cable and a reachable test peer. The app does not join networks or change network configuration."),
        ("bluetooth_device", "Known working Bluetooth peripheral", "Connect a peripheral using the desktop's Bluetooth settings, then exercise its function."),
        ("external_microphone", "Known working external microphone", "Needed for acoustic speaker verification when no usable physical microphone exists. Never select a software monitor or loopback input."),
        ("optical_disc", "Known working optical disc", "Use readable media compatible with the drive."),
        ("firewire_device", "FireWire device and cable", "Use a known working native FireWire device; test a data transfer through the target connector."),
        ("thunderbolt_device", "Thunderbolt device and cable", "Use a genuine Thunderbolt device and compatible cable. A USB device in the same connector does not verify Thunderbolt."),
    ].into_iter().map(|(id,name,instructions)| Equipment { id: id.into(), name: name.into(), instructions: instructions.into(), required_for: vec![], readiness: Readiness::Uncertain }).collect()
}

pub fn build(
    inventory: &Inventory,
    profile: Profile,
    mode: Mode,
    readiness: &BTreeMap<String, Readiness>,
    selected: Option<&BTreeSet<String>>,
) -> TestPlan {
    let extended = profile == Profile::Extended;
    let mut tests = Vec::new();
    for d in &inventory.devices {
        let mut add = |suffix: &str,
                       title: &str,
                       probe: &str,
                       guided: bool,
                       equipment: &[&str],
                       instructions: &str,
                       seconds: u64| {
            let id = format!("{}/{}", d.id, suffix);
            if selected.is_some_and(|s| !s.contains(&id)) {
                return;
            }
            tests.push(TestSpec { id, device_id: d.id.clone(), name: format!("{} — {title}", d.name), probe: probe.into(), guided, required: true,
                equipment: equipment.iter().map(|s| (*s).into()).collect(),
                permission: match probe { "storage_health" | "storage_read" | "media_read" => Some("Read access to the selected block device; optional policy-controlled helper for SMART".into()), "camera" | "camera_quality" | "microphone" | "microphone_speech" | "acoustic" => Some("Camera/microphone device access".into()), "keyboard_backlight" => Some("UPower keyboard-backlight control on the system bus".into()), _ => None },
                instructions: instructions.into(), estimated_seconds: seconds, deadline_seconds: seconds.saturating_add(15).max(20) });
        };
        if d.recognition == Recognition::ExpectedNotDetected {
            let kit: &[&str] = match d.kind {
                Kind::SdReader => &["sd_card"],
                Kind::AudioJack => &["headset"],
                Kind::Firewire => &["firewire_device"],
                Kind::Thunderbolt => &["thunderbolt_device"],
                Kind::Optical => &["optical_disc"],
                _ => &[],
            };
            add(
                "availability",
                "Expected component",
                "availability",
                false,
                kit,
                "Record absence and driver evidence; do not infer a physical defect.",
                1,
            );
            continue;
        }
        match d.kind {
            Kind::Cpu => add(
                "compute",
                "CPU computation",
                "cpu",
                false,
                &[],
                "Run deterministic calculations; extended mode adds sustained load.",
                if extended { 120 } else { 2 },
            ),
            Kind::Memory => add(
                "patterns",
                "Allocated-memory patterns",
                "memory",
                false,
                &[],
                "Test bounded allocated memory. This does not test all physical RAM.",
                if extended { 120 } else { 3 },
            ),
            Kind::Storage => {
                add(
                    "health",
                    "SMART/NVMe health",
                    "storage_health",
                    false,
                    &[],
                    "Read health telemetry without starting repairs or modifying SMART settings.",
                    8,
                );
                add(
                    "read",
                    "Read-only sample",
                    "storage_read",
                    false,
                    &[],
                    "Read bounded samples without mounting filesystems or writing to the disk.",
                    if extended { 120 } else { 4 },
                );
            }
            Kind::Usb => {
                if d.attributes.get("role").is_none_or(|v| v == "root_hub") {
                    add(
                        "port",
                        "USB connector and media",
                        "usb_port",
                        true,
                        &["usb_media"],
                        "Unplug the test media first. When prompted, insert it into one physical port, enter that port's label and mounted fixture file path. Repeat this test for each port. Adapters and controller discovery do not prove every connector works.",
                        15,
                    );
                }
            }
            Kind::SdReader => add(
                "card",
                "SD reader and media",
                "sd_card",
                true,
                &["sd_card"],
                "Remove the card first. When prompted, insert it into this reader and enter the mounted fixture file path.",
                15,
            ),
            Kind::Camera => {
                add(
                    "capture",
                    "Camera capture",
                    "camera",
                    false,
                    &[],
                    "Capture several frames in memory and compare image content.",
                    4,
                );
                add(
                    "quality",
                    "Camera image quality",
                    "camera_quality",
                    true,
                    &[],
                    "Look at the preview, move in front of the camera, and assess focus, colour, obstruction and artifacts.",
                    4,
                );
            }
            Kind::Microphone => {
                add(
                    "capture",
                    "Physical microphone capture",
                    "microphone",
                    false,
                    &[],
                    "Measure a short PCM recording; no recording is retained.",
                    2,
                );
                add(
                    "speech",
                    "Microphone response",
                    "microphone_speech",
                    true,
                    &[],
                    "Speak clearly near this microphone for three seconds. The captured sample is then played once through the analog output; listen for your voice rather than room noise. Audio is not saved.",
                    3,
                );
            }
            Kind::Speaker => {
                if d.attributes
                    .get("playback_route")
                    .is_some_and(|route| route == "display")
                {
                    add(
                        "playback",
                        "Digital display audio stream",
                        "speaker",
                        false,
                        &["external_display"],
                        "Connect an audio-capable display to the relevant output. Stream delivery alone does not prove audible sound.",
                        3,
                    );
                    add(
                        "quality",
                        "Digital display audio assessment",
                        "speaker_quality",
                        true,
                        &["external_display"],
                        "Listen through the connected audio-capable display and confirm the selected route. Do not assess this endpoint as the Mac's built-in speaker.",
                        3,
                    );
                    continue;
                }
                add(
                    "playback",
                    "Left/right playback",
                    "speaker",
                    false,
                    &[],
                    "Play test-only left/right tones through this explicit ALSA output. The system mixer setting is unchanged; stream delivery alone does not prove audible sound.",
                    3,
                );
                let eq = if inventory.devices.iter().any(|m| {
                    m.kind == Kind::Microphone
                        && m.recognition == Recognition::Detected
                        && m.attributes
                            .get("physical_input")
                            .is_none_or(|v| v == "true")
                }) {
                    vec![]
                } else {
                    vec!["external_microphone"]
                };
                add(
                    "acoustic",
                    "Acoustic speaker verification",
                    "acoustic",
                    false,
                    &eq,
                    "Select a physical microphone. Record baseline noise and correlate the known left/right test sequence. Do not use software loopback; inconclusive results need independent checks.",
                    6,
                );
                add(
                    "quality",
                    "Speaker sound quality",
                    "speaker_quality",
                    true,
                    &[],
                    "During playback listen to each channel for sound, rattling, distortion and channel imbalance. Confirm the physical output route.",
                    3,
                );
            }
            Kind::AudioJack => add(
                "headset",
                "Headset and jack routing",
                "audio_jack",
                true,
                &["headset"],
                "Insert the compatible headset when prompted. Listen to the selected output, verify speaker-to-headset routing, and test microphone input only if this jack supports it.",
                5,
            ),
            Kind::Gpu => add(
                "render",
                "Hardware render/readback",
                "gpu",
                false,
                &[],
                "Select this GPU's EGL device, render colour quadrants and verify pixel readback. Software rendering cannot pass.",
                if extended { 60 } else { 2 },
            ),
            Kind::Display => {
                add(
                    "connection",
                    "Display connector/EDID",
                    "display",
                    false,
                    &[],
                    "Check the connector and EDID; disconnected connectors remain unverified.",
                    1,
                );
                let external = d.attributes.get("internal").is_none_or(|v| v != "true");
                add(
                    "visual",
                    "Display patterns",
                    "manual",
                    true,
                    if external { &["external_display"] } else { &[] },
                    "Select the matching screen, show colour/gradient/pixel patterns, and check dead pixels, flicker, lines and brightness. A rendered pattern requires visual confirmation.",
                    15,
                );
            }
            Kind::Battery => {
                add(
                    "health",
                    "Battery telemetry",
                    "battery",
                    false,
                    &[],
                    "Read capacity, wear and cycle count when exposed.",
                    1,
                );
                add(
                    "charging",
                    "Charging transition",
                    "charging",
                    true,
                    &["charger"],
                    "Start with the charger disconnected. When prompted connect it, wait for telemetry to update, then continue. Confirm charging or a legitimate full-battery state.",
                    8,
                );
            }
            Kind::Power => add(
                "supply",
                "Supply telemetry",
                "power",
                false,
                &[],
                "Read external supply presence.",
                1,
            ),
            Kind::Network => {
                add(
                    "link",
                    "Interface/link",
                    "network",
                    false,
                    &[],
                    "Check interface state; no network configuration is changed.",
                    1,
                );
                add(
                    "peer",
                    "Network peer exchange",
                    "network_peer",
                    true,
                    &["network_peer"],
                    "Connect this interface to the test network using desktop settings. Enter the designated IPv4/IPv6 peer address.",
                    5,
                );
            }
            Kind::Bluetooth => add(
                "peripheral",
                "Bluetooth peripheral",
                "bluetooth",
                true,
                &["bluetooth_device"],
                "Connect and exercise the test peripheral, then enter its Bluetooth MAC address. Existing connections are inspected; pairing is not modified.",
                5,
            ),
            Kind::Keyboard => add(
                "keys",
                "Keyboard coverage",
                "manual",
                true,
                &[],
                "Exercise every key, including modifiers and function keys. Record tested keys and any missing keys; confirm coverage explicitly.",
                30,
            ),
            Kind::Trackpad => add(
                "input",
                "Trackpad functions",
                "manual",
                true,
                &[],
                "Exercise movement, left/right click, dragging and scrolling; use the input exercise area.",
                15,
            ),
            Kind::Thermal | Kind::Fan => add(
                "telemetry",
                "Sensor telemetry",
                "sensor",
                false,
                &[],
                "Read sensor telemetry. A zero RPM reading may reflect idle fan control, not a broken fan.",
                1,
            ),
            Kind::Optical => add(
                "disc",
                "Optical read",
                "media_read",
                true,
                &["optical_disc"],
                "Insert the known working disc and read a bounded sample.",
                10,
            ),
            Kind::Firewire => add(
                "device",
                "FireWire function",
                "manual",
                true,
                &["firewire_device"],
                "Connect a FireWire device and perform a known data transfer. Record the peripheral and operation.",
                20,
            ),
            Kind::Thunderbolt => add(
                "device",
                "Thunderbolt link",
                "thunderbolt",
                true,
                &["thunderbolt_device"],
                "Connect the genuine test device using desktop authorization if required, then perform a known data transfer. A controller or USB connection alone is insufficient.",
                20,
            ),
            Kind::KeyboardBacklight => add(
                "visual",
                "Keyboard backlight brightness",
                "keyboard_backlight",
                true,
                &[],
                "Watch the keyboard while the light turns off, then bright, then returns to its original level. Confirm whether the keys visibly changed; a control readback alone is not a visual pass.",
                4,
            ),
            Kind::TouchBar => add(
                "visual",
                "Visual/input function",
                "manual",
                true,
                &[],
                "Exercise the component using supported desktop controls and record observed behaviour.",
                10,
            ),
        }
    }
    let mut equipment = preparation();
    for item in &mut equipment {
        item.required_for = tests
            .iter()
            .filter(|t| t.equipment.contains(&item.id))
            .map(|t| t.id.clone())
            .collect();
        item.readiness = readiness.get(&item.id).copied().unwrap_or_default();
    }
    equipment.retain(|e| !e.required_for.is_empty());
    let seconds = tests.iter().map(|t| t.estimated_seconds).sum();
    TestPlan { profile, mode, tests, equipment, estimated_seconds: seconds,
        notices: vec!["Equipment requirements must be reviewed before functional tests. Missing items leave explicit coverage gaps.".into(),
            "Audio playback is audible. Camera/microphone frames stay transient and are excluded from reports.".into(),
            "Linux driver failures and missing firmware are not proof of defective hardware.".into(),
            "Physical port coverage needs operator labels. Repeat connector tests for each physical port; port layout is not inferred from root hubs.".into(),
            "Extended tests are bounded samples, not certification of all RAM, VRAM, disk sectors or battery runtime.".into()] }
}

pub fn validate(
    inventory: &Inventory,
    profile: Profile,
    mode: Mode,
    readiness: &BTreeMap<String, Readiness>,
    selected: Option<&BTreeSet<String>>,
) -> Result<(), String> {
    let known_equipment: BTreeSet<_> = preparation().into_iter().map(|e| e.id).collect();
    for id in readiness.keys() {
        if !known_equipment.contains(id) {
            return Err(format!("Unknown equipment ID: {id}"));
        }
    }
    if let Some(selected) = selected {
        let catalogue = build(inventory, profile, mode, readiness, None);
        for id in selected {
            if !catalogue.tests.iter().any(|t| &t.id == id) {
                return Err(format!("Unknown test ID: {id}"));
            }
        }
    }
    Ok(())
}
