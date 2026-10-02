use crate::model::*;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

// Epoch milliseconds are UTC and preserve ordering without depending on the host locale.
pub fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
pub fn session_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        timestamp(),
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

impl Report {
    pub fn new(inventory: Inventory, plan: TestPlan) -> Self {
        let mut report = Self {
            schema_version: SCHEMA_VERSION,
            application_version: env!("CARGO_PKG_VERSION").into(),
            session_id: session_id(),
            started_at: timestamp(),
            finished_at: None,
            state: "prepared".into(),
            overall: "incomplete".into(),
            inventory,
            plan,
            results: vec![],
            coverage: Coverage {
                selected: 0,
                completed: 0,
                passed: 0,
                failed: 0,
                incomplete: 0,
                gaps: vec![],
            },
        };
        report.refresh();
        report
    }
    pub fn refresh(&mut self) {
        self.coverage = Coverage {
            selected: self.plan.tests.len(),
            completed: self.results.len(),
            passed: self
                .results
                .iter()
                .filter(|r| r.outcome == Outcome::Passed)
                .count(),
            failed: self
                .results
                .iter()
                .filter(|r| r.outcome == Outcome::Failed)
                .count(),
            incomplete: 0,
            gaps: vec![],
        };
        for test in &self.plan.tests {
            if !test.required {
                // Historical retake attempts remain in results but do not create
                // a coverage gap after a later attempt is completed.
                continue;
            }
            match self.results.iter().find(|r| r.test_id == test.id) {
                Some(r) if matches!(r.outcome, Outcome::Passed | Outcome::Failed) => {}
                Some(r) => {
                    self.coverage.incomplete += 1;
                    self.coverage
                        .gaps
                        .push(format!("{}: {}", test.id, r.reason_code));
                }
                None => {
                    self.coverage.incomplete += 1;
                    self.coverage.gaps.push(format!("{}: pending", test.id));
                }
            }
        }
        let expected_usb = self
            .inventory
            .machine
            .expected_ports
            .get("usb_a")
            .copied()
            .unwrap_or(0)
            + self
                .inventory
                .machine
                .expected_ports
                .get("usb_c")
                .copied()
                .unwrap_or(0);
        let labels: std::collections::BTreeSet<_> = self
            .results
            .iter()
            .filter(|r| r.outcome == Outcome::Passed)
            .filter_map(|r| {
                r.measurements
                    .get("physical_port_label")
                    .and_then(|m| m.value.as_str())
            })
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if expected_usb > 0 && labels.len() < expected_usb as usize {
            self.coverage.gaps.push(format!("usb_physical_ports: {} distinct verified operator labels of {} published connectors; remaining connectors unverified",labels.len(),expected_usb));
        } else if expected_usb == 0 && self.plan.tests.iter().any(|t| t.probe == "usb_port") {
            self.coverage.gaps.push("physical_port_layout_unknown: controller checks cannot establish total physical port coverage".into());
        }
        let required_pass = !self.plan.tests.is_empty()
            && self.plan.tests.iter().filter(|t| t.required).all(|t| {
                self.results
                    .iter()
                    .any(|r| r.test_id == t.id && r.outcome == Outcome::Passed)
            });
        self.overall = if self.coverage.failed > 0 {
            "failed"
        } else if required_pass && self.state == "completed" {
            "passed"
        } else {
            "incomplete"
        }
        .into();
    }
    pub fn upsert(&mut self, result: TestResult) {
        if let Some(old) = self
            .results
            .iter_mut()
            .find(|r| r.test_id == result.test_id)
        {
            *old = result;
        } else {
            self.results.push(result);
        }
        self.refresh();
    }
}

pub fn write_atomic(path: &Path, report: &Report) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .ok_or("Report path needs a file name")?
        .to_string_lossy();
    let tmp = parent.join(format!(".{name}.{}.tmp", report.session_id));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut file, report).map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
