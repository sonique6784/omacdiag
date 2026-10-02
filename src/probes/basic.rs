use super::*;
use crate::discovery::{self, read};
use serde_json::json;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};

pub fn execute(req: &WorkerRequest, d: &Device) -> TestResult {
    match req.test.probe.as_str() {
        "cpu" => cpu(req),
        "memory" => memory(req),
        "storage_health" => storage_health(req, d),
        "storage_read" | "media_read" => storage_read(req, d),
        "sensor" => sensor(req, d),
        "battery" => battery(req, d),
        "power" => power(req, d),
        "charging" => charging(req, d),
        "display" => display(req, d),
        "network" => network(req, d),
        "network_peer" => network_peer(req, d),
        "bluetooth" => bluetooth(req, d),
        "usb_port" | "sd_card" => removable(req, d),
        "thunderbolt" => thunderbolt(req, d),
        _ => problem(req, "Unknown basic probe"),
    }
}
fn cpu(req: &WorkerRequest) -> TestResult {
    let duration = Duration::from_secs(if req.profile == Profile::Extended {
        120
    } else {
        2
    });
    let start = Instant::now();
    let mut iterations = 0_u64;
    // A modular arithmetic identity checks the same work via a different computation.
    while start.elapsed() < duration {
        let n = 10_000_u64;
        let mut sum = 0_u64;
        for i in 1..=n {
            sum = std::hint::black_box(sum.wrapping_add(std::hint::black_box(i)));
        }
        if sum != n * (n + 1) / 2 {
            return TestResult::new(
                &req.test,
                Outcome::Failed,
                "computation_mismatch",
                "CPU computation returned an incorrect result",
            );
        }
        iterations += 1;
        if iterations.is_multiple_of(100) && thermal_limit() {
            return TestResult::new(
                &req.test,
                Outcome::Inconclusive,
                "thermal_limit",
                "Stopped workload at 90°C sensor threshold",
            );
        }
    }
    let mut r = TestResult::new(
        &req.test,
        Outcome::Passed,
        "computation_verified",
        "Deterministic CPU calculations completed correctly",
    );
    r.measure("iterations", iterations, "count");
    r.measure("seconds", start.elapsed().as_secs_f64(), "s");
    r.evidence.push(
        "One worker; this is not an all-core burn-in or instruction-set certification".into(),
    );
    r
}
pub fn thermal_limit() -> bool {
    discovery::entries("/sys/class/hwmon")
        .iter()
        .flat_map(discovery::entries)
        .filter(|p| {
            p.file_name().is_some_and(|n| {
                n.to_string_lossy().starts_with("temp") && n.to_string_lossy().ends_with("_input")
            })
        })
        .any(|p| {
            read(p)
                .and_then(|s| s.parse::<i64>().ok())
                .is_some_and(|v| v >= 90_000)
        })
}
fn memory(req: &WorkerRequest) -> TestResult {
    let available = read("/proc/meminfo")
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("MemAvailable:")
                    .and_then(|s| s.split_whitespace().next()?.parse::<usize>().ok())
            })
        })
        .unwrap_or(128 * 1024)
        * 1024;
    let bytes = (available / 8).min(if req.profile == Profile::Extended {
        256 * 1024 * 1024
    } else {
        32 * 1024 * 1024
    });
    if bytes < 1024 * 1024 {
        return problem(req, "Insufficient free memory for a bounded allocation");
    }
    let words = bytes / 8;
    let mut buffer = Vec::<u64>::new();
    if buffer.try_reserve_exact(words).is_err() {
        return problem(req, "Memory allocation refused");
    }
    buffer.resize(words, 0);
    let start = Instant::now();
    let duration = Duration::from_secs(if req.profile == Profile::Extended {
        120
    } else {
        3
    });
    let mut passes = 0;
    loop {
        for pattern in [0_u64, u64::MAX, 0xaaaaaaaaaaaaaaaa, 0x5555555555555555] {
            for (i, w) in buffer.iter_mut().enumerate() {
                *w = std::hint::black_box(pattern ^ i as u64);
            }
            for (i, w) in buffer.iter().enumerate() {
                if std::hint::black_box(*w) != pattern ^ i as u64 {
                    return TestResult::new(
                        &req.test,
                        Outcome::Failed,
                        "memory_mismatch",
                        format!("Pattern mismatch at allocated word {i}"),
                    );
                }
            }
            if start.elapsed() >= duration {
                break;
            }
        }
        passes += 1;
        if start.elapsed() >= duration {
            break;
        }
        if thermal_limit() {
            return TestResult::new(
                &req.test,
                Outcome::Inconclusive,
                "thermal_limit",
                "Memory workload stopped at thermal threshold",
            );
        }
    }
    let mut r = TestResult::new(
        &req.test,
        Outcome::Passed,
        "allocated_patterns_verified",
        "Patterns verified in allocated userspace memory",
    );
    r.measure("allocated_bytes", bytes, "bytes");
    r.measure("passes", passes, "count");
    r.evidence
        .push("Unallocated RAM and inaccessible memory regions were not tested".into());
    r
}

fn storage_health(req: &WorkerRequest, d: &Device) -> TestResult {
    let Some(node) = &d.node else {
        return problem(req, "No block endpoint");
    };
    let output = match run("smartctl", &["--json", "--all", node], None, 8, 512 * 1024) {
        Ok(o) => o,
        Err(e) => return problem(req, e),
    };
    let data: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return problem(req, output.stderr),
    };
    let code = output.code.unwrap_or(1);
    if code & 7 != 0 {
        // Optional helper takes only a validated kernel block-device basename.
        let base = Path::new(node)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        if std::env::var_os("OMACDIAG_USE_HELPER").is_some()
            && let Ok(helper) = run(
                "pkexec",
                &["/usr/libexec/omacdiag-helper", "smart", &base],
                None,
                8,
                512 * 1024,
            )
            && let Ok(v) = serde_json::from_slice(&helper.stdout)
        {
            return evaluate_health(req, v, helper.code.unwrap_or(1));
        }
        let mut r = TestResult::new(
            &req.test,
            Outcome::Blocked,
            "health_query_unavailable",
            "SMART unavailable or access denied; no health verdict is possible",
        );
        r.measure("smartctl_exit_status", code, "bitmask");
        r.evidence.push(data.to_string());
        return r;
    }
    evaluate_health(req, data, code)
}
fn evaluate_health(req: &WorkerRequest, data: serde_json::Value, code: i32) -> TestResult {
    let passed = data
        .pointer("/smart_status/passed")
        .and_then(|v| v.as_bool());
    let critical = data
        .pointer("/nvme_smart_health_information_log/critical_warning")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let (status, reason) = if passed == Some(false) || critical != 0 || code & 0x38 != 0 {
        (Outcome::Failed, "storage_health_warning")
    } else if passed == Some(true) {
        (Outcome::Passed, "reported_health_passed")
    } else {
        (Outcome::Inconclusive, "health_not_reported")
    };
    let mut r = TestResult::new(
        &req.test,
        status,
        reason,
        "Drive-reported health; historical error and self-test logs are preserved as evidence",
    );
    r.measure("smartctl_exit_status", code, "bitmask");
    r.measure("drive_health", data, "structured");
    r
}
fn storage_read(req: &WorkerRequest, d: &Device) -> TestResult {
    let Some(node) = &d.node else {
        return problem(req, "No block endpoint");
    };
    let mut f = match File::open(node) {
        Ok(f) => f,
        Err(e) => return problem(req, e.to_string()),
    };
    let bytes = d
        .attributes
        .get("size")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
        .saturating_mul(512);
    if bytes == 0 {
        return TestResult::new(
            &req.test,
            Outcome::Inconclusive,
            "media_capacity_unavailable",
            "No readable media capacity is exposed",
        );
    }
    let sample = (1024 * 1024_u64).min(bytes);
    let count = if req.profile == Profile::Extended {
        128
    } else {
        4
    };
    let mut buffer = vec![0_u8; sample as usize];
    let start = Instant::now();
    let mut total = 0_u64;
    for i in 0..count {
        let offset = if count > 1 {
            ((bytes - sample) / (count - 1) * i) / 512 * 512
        } else {
            0
        };
        if let Err(e) = f
            .seek(SeekFrom::Start(offset))
            .and_then(|_| f.read_exact(&mut buffer))
        {
            let mut r = TestResult::new(
                &req.test,
                Outcome::Failed,
                "read_error",
                format!("Read failed at byte {offset}: {e}"),
            );
            r.measure("bytes_read", total, "bytes");
            return r;
        }
        total += sample;
        if start.elapsed().as_secs() >= req.test.estimated_seconds {
            break;
        }
    }
    let mut r = TestResult::new(
        &req.test,
        Outcome::Passed,
        "sample_readable",
        "Read-only distributed samples completed; unsampled sectors remain unverified",
    );
    r.measure("bytes_read", total, "bytes");
    r.measure("capacity", bytes, "bytes");
    r.measure("elapsed", start.elapsed().as_secs_f64(), "s");
    r
}
fn sensor(req: &WorkerRequest, d: &Device) -> TestResult {
    let value = d
        .attributes
        .get("sensor_file")
        .and_then(read)
        .and_then(|s| s.parse::<i64>().ok());
    let Some(value) = value else {
        return problem(req, "Sensor value unavailable");
    };
    let mut r = TestResult::new(
        &req.test,
        Outcome::Inconclusive,
        "telemetry_only",
        "Telemetry was read; a value alone does not establish sensor accuracy or fan function",
    );
    r.measure(
        "reading",
        if d.kind == Kind::Thermal {
            value as f64 / 1000.0
        } else {
            value as f64
        },
        if d.kind == Kind::Thermal {
            "°C"
        } else {
            "RPM"
        },
    );
    r
}
fn battery(req: &WorkerRequest, d: &Device) -> TestResult {
    let Some(path) = d.sysfs_path.as_ref() else {
        return problem(req, "No battery endpoint");
    };
    let mut r = TestResult::new(
        &req.test,
        Outcome::Inconclusive,
        "battery_telemetry_only",
        "Capacity and wear are observations; usable runtime needs a separate discharge test",
    );
    for key in [
        "capacity",
        "cycle_count",
        "energy_full",
        "energy_full_design",
        "charge_full",
        "charge_full_design",
    ] {
        if let Some(v) = read(Path::new(path).join(key)).and_then(|s| s.parse::<f64>().ok()) {
            r.measure(
                key,
                v,
                match key {
                    "capacity" => "%",
                    "cycle_count" => "count",
                    s if s.starts_with("energy") => "µWh",
                    _ => "µAh",
                },
            );
        }
    }
    for (full, design) in [
        ("energy_full", "energy_full_design"),
        ("charge_full", "charge_full_design"),
    ] {
        if let (Some(f), Some(n)) = (
            r.measurements.get(full).and_then(|m| m.value.as_f64()),
            r.measurements.get(design).and_then(|m| m.value.as_f64()),
        ) && n > 0.0
        {
            r.measure("remaining_design_capacity", f / n * 100.0, "%");
            break;
        }
    }
    r
}
fn power(req: &WorkerRequest, d: &Device) -> TestResult {
    let online = d
        .sysfs_path
        .as_ref()
        .and_then(|p| read(Path::new(p).join("online")));
    let mut r = TestResult::new(
        &req.test,
        if online.as_deref() == Some("1") {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if online.as_deref() == Some("1") {
            "supply_present"
        } else {
            "supply_not_online"
        },
        "External supply presence does not prove charging or power delivery under load",
    );
    r.measure("online", online, "state");
    r
}
fn charging(req: &WorkerRequest, d: &Device) -> TestResult {
    let after = d
        .sysfs_path
        .as_ref()
        .and_then(|p| read(Path::new(p).join("status")))
        .unwrap_or("unknown".into());
    let before = req
        .baseline
        .devices
        .iter()
        .find(|b| b.id == d.id)
        .and_then(|b| b.attributes.get("status"))
        .cloned()
        .unwrap_or("unknown".into());
    let transition = before != after && (after == "Charging" || after == "Full");
    let mut r = TestResult::new(
        &req.test,
        if transition {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if transition {
            "charging_transition_observed"
        } else {
            "charging_transition_unverified"
        },
        "Charging state compared before and after the technician's action",
    );
    r.measure("before", before, "state");
    r.measure("after", after, "state");
    r.observation = req.observation.clone();
    r
}
fn display(req: &WorkerRequest, d: &Device) -> TestResult {
    let Some(path) = &d.sysfs_path else {
        return problem(req, "No DRM connector");
    };
    let status = read(Path::new(path).join("status")).unwrap_or("unknown".into());
    let edid = std::fs::read(Path::new(path).join("edid")).unwrap_or_default();
    let checksum = !edid.is_empty()
        && edid.len().is_multiple_of(128)
        && edid
            .chunks(128)
            .all(|chunk| chunk.iter().fold(0_u8, |sum, b| sum.wrapping_add(*b)) == 0);
    let mut r = TestResult::new(
        &req.test,
        if status == "connected" && checksum {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if status == "connected" && checksum {
            "connector_and_edid_valid"
        } else {
            "display_link_unverified"
        },
        "Connector/EDID check; panel pixels, image quality and brightness require visual inspection",
    );
    r.measure("connector_status", status, "state");
    r.measure("edid_bytes", edid.len(), "bytes");
    r.measure("edid_checksums_valid", checksum, "boolean");
    r
}
fn network(req: &WorkerRequest, d: &Device) -> TestResult {
    let carrier = d
        .sysfs_path
        .as_ref()
        .and_then(|p| read(Path::new(p).join("carrier")));
    let mut r = TestResult::new(
        &req.test,
        if carrier.as_deref() == Some("1") {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if carrier.as_deref() == Some("1") {
            "link_present"
        } else {
            "link_not_present"
        },
        "Link presence does not prove packet exchange or internet access",
    );
    r.measure("carrier", carrier, "state");
    r
}
fn network_peer(req: &WorkerRequest, d: &Device) -> TestResult {
    let peer = req
        .observation
        .as_ref()
        .and_then(|o| o.details.get("peer"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if peer.parse::<std::net::IpAddr>().is_err() {
        return problem(req, "Enter a designated numeric IPv4/IPv6 peer address");
    }
    let interface = d
        .sysfs_path
        .as_ref()
        .and_then(|p| Path::new(p).file_name())
        .unwrap_or_default()
        .to_string_lossy();
    match run(
        "ping",
        &["-n", "-I", &interface, "-c", "3", "-W", "1", peer],
        None,
        5,
        16384,
    ) {
        Ok(o) => {
            let mut r = TestResult::new(
                &req.test,
                if o.code == Some(0) {
                    Outcome::Passed
                } else {
                    Outcome::Inconclusive
                },
                if o.code == Some(0) {
                    "peer_exchange_verified"
                } else {
                    "peer_unreachable"
                },
                "ICMP exchange through the selected interface; blocked ICMP does not establish a hardware fault",
            );
            r.evidence.push(String::from_utf8_lossy(&o.stdout).into());
            r.observation = req.observation.clone();
            r
        }
        Err(e) => problem(req, e),
    }
}
fn bluetooth(req: &WorkerRequest, d: &Device) -> TestResult {
    let address = req
        .observation
        .as_ref()
        .and_then(|o| o.details.get("address"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if address.len() != 17
        || !address
            .split(':')
            .all(|s| s.len() == 2 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return problem(req, "Enter the test peripheral Bluetooth MAC address");
    }
    let adapter = d
        .sysfs_path
        .as_ref()
        .and_then(|p| Path::new(p).file_name())
        .unwrap_or_default()
        .to_string_lossy();
    if !adapter.starts_with("hci") || !adapter[3..].bytes().all(|b| b.is_ascii_digit()) {
        return problem(req, "BlueZ adapter mapping unavailable");
    }
    let object = format!(
        "/org/bluez/{adapter}/dev_{}",
        address.replace(':', "_").to_uppercase()
    );
    match run(
        "busctl",
        &[
            "--timeout=5",
            "get-property",
            "org.bluez",
            &object,
            "org.bluez.Device1",
            "Connected",
        ],
        None,
        5,
        32768,
    ) {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let connected = o.code == Some(0) && text.trim() == "b true";
            let mut r = if connected {
                TestResult::new(
                    &req.test,
                    Outcome::Passed,
                    "peripheral_connected",
                    "Peripheral is connected on the selected adapter; function still requires operator assessment",
                )
            } else {
                TestResult::new(
                    &req.test,
                    Outcome::Inconclusive,
                    "peripheral_not_connected",
                    "No connected test peripheral was observed on the selected adapter",
                )
            };
            r.evidence.push(object);
            r.evidence.push(text.into());
            r
        }
        Err(e) => problem(req, e),
    }
}
fn removable(req: &WorkerRequest, d: &Device) -> TestResult {
    let fresh = discovery::discover();
    let added: Vec<_> = fresh
        .devices
        .iter()
        .filter(|n| !req.baseline.devices.iter().any(|b| b.id == n.id))
        .collect();
    let kind_ok = if req.test.probe == "usb_port" {
        let controller = d
            .sysfs_path
            .as_ref()
            .and_then(|s| std::fs::canonicalize(s).ok());
        added.iter().any(|n| {
            n.kind == Kind::Usb
                && n.attributes.get("role").is_some_and(|v| v == "device")
                && controller.as_ref().is_some_and(|parent| {
                    n.sysfs_path
                        .as_ref()
                        .and_then(|p| std::fs::canonicalize(p).ok())
                        .is_some_and(|p| p.starts_with(parent))
                })
        })
    } else {
        added
            .iter()
            .any(|n| n.kind == Kind::Storage || n.kind == Kind::SdReader)
    };
    if !kind_ok {
        return TestResult::new(
            &req.test,
            Outcome::Inconclusive,
            "insertion_not_observed",
            "No new matching peripheral was observed. Unplug it, repeat, and insert only after the prompt",
        );
    }
    let details = req
        .observation
        .as_ref()
        .map(|o| &o.details)
        .unwrap_or(&serde_json::Value::Null);
    let fixture = details
        .get("fixture_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !Path::new(fixture).is_absolute() {
        return problem(
            req,
            "Supply an absolute mounted fixture file path; the app does not mount media",
        );
    }
    let label = details
        .get("port_label")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if req.test.probe == "usb_port" && label.trim().is_empty() {
        return problem(req, "A physical port label is required");
    }
    let mut file = match File::open(fixture) {
        Ok(f) => f,
        Err(e) => return problem(req, e.to_string()),
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return problem(req, "Fixture must be a regular file");
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let dev = file.metadata().unwrap().dev();
        let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & 0xfffff000);
        let minor = (dev & 0xff) | ((dev >> 12) & 0xffffff00);
        let mounted = std::fs::canonicalize(format!("/sys/dev/block/{major}:{minor}"));
        let belongs_to_added = mounted.as_ref().is_ok_and(|mount| {
            added
                .iter()
                .filter(|n| n.kind == Kind::Storage || n.kind == Kind::SdReader)
                .any(|n| {
                    n.sysfs_path
                        .as_ref()
                        .and_then(|p| std::fs::canonicalize(p).ok())
                        .is_some_and(|p| mount.starts_with(p))
                })
        });
        let controller = d
            .sysfs_path
            .as_ref()
            .and_then(|p| std::fs::canonicalize(p).ok());
        let belongs_to_reader = mounted
            .as_ref()
            .is_ok_and(|mount| controller.as_ref().is_some_and(|p| mount.starts_with(p)));
        if !belongs_to_added || !belongs_to_reader {
            return TestResult::new(
                &req.test,
                Outcome::Blocked,
                "fixture_device_mismatch",
                "Fixture file must be on the newly inserted media under the selected controller/reader; system-disk or unrelated-media fixtures cannot pass",
            );
        }
    }
    let mut data = Vec::new();
    if let Err(e) = Read::by_ref(&mut file)
        .take((FIXTURE_BYTES + 1) as u64)
        .read_to_end(&mut data)
    {
        return problem(req, e.to_string());
    }
    if data != fixture_data() {
        return TestResult::new(
            &req.test,
            Outcome::Failed,
            "fixture_mismatch",
            "Fixture content differs from the generated reference; media, filesystem and path causes need investigation",
        );
    }
    let mut r = TestResult::new(
        &req.test,
        Outcome::Passed,
        "insertion_and_fixture_verified",
        "Insertion, selected controller ancestry, mounted-media identity and known content verified",
    );
    r.measure("fixture_bytes", data.len(), "bytes");
    r.measure("physical_port_label", label, "label");
    r.measure(
        "new_devices",
        added.iter().map(|n| n.id.clone()).collect::<Vec<_>>(),
        "identifiers",
    );
    r.observation = req.observation.clone();
    r
}
fn thunderbolt(req: &WorkerRequest, d: &Device) -> TestResult {
    let current = discovery::discover();
    let label = d
        .sysfs_path
        .as_ref()
        .and_then(|p| Path::new(p).file_name())
        .unwrap_or_default()
        .to_string_lossy();
    let domain = label
        .strip_prefix("domain")
        .unwrap_or_else(|| label.split('-').next().unwrap_or(""));
    if domain.is_empty() || !domain.bytes().all(|b| b.is_ascii_digit()) {
        return problem(req, "Thunderbolt domain mapping unavailable");
    }
    let linked: Vec<_> = current
        .devices
        .iter()
        .filter(|d| {
            d.kind == Kind::Thunderbolt
                && d.id.starts_with(&format!("thunderbolt:{domain}-"))
                && d.attributes.get("authorized").is_some_and(|v| v == "1")
                && !req.baseline.devices.iter().any(|b| b.id == d.id)
        })
        .collect();
    let mut r = if linked.is_empty() {
        TestResult::new(
            &req.test,
            Outcome::Inconclusive,
            "thunderbolt_link_unverified",
            "No newly authorized Thunderbolt peripheral was observed",
        )
    } else {
        TestResult::new(
            &req.test,
            Outcome::Passed,
            "thunderbolt_link_observed",
            "A newly authorized peripheral is present on the selected domain; data transfer still requires operator assessment",
        )
    };
    r.measure(
        "new_authorized_devices",
        json!(linked.iter().map(|d| &d.id).collect::<Vec<_>>()),
        "identifiers",
    );
    r
}
