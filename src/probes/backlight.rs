//! Bounded keyboard-backlight cycle through UPower. The cycle runs in a separate
//! process group so cancellation of the diagnostic worker does not interrupt
//! the attempt to restore the original brightness.
use super::*;
use crate::discovery::read;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

const SERVICE: &str = "org.freedesktop.UPower";
const OBJECT: &str = "/org/freedesktop/UPower/KbdBacklight";
const INTERFACE: &str = "org.freedesktop.UPower.KbdBacklight";

#[derive(Serialize, Deserialize)]
struct CycleRequest {
    sysfs_path: PathBuf,
    sysfs_target: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct CycleResult {
    original: i32,
    low: i32,
    high: i32,
    restored: i32,
    maximum: i32,
}

pub fn execute(req: &WorkerRequest, device: &Device) -> TestResult {
    let Some(path) = device.sysfs_path.as_ref() else {
        return TestResult::new(
            &req.test,
            Outcome::Blocked,
            "backlight_path_missing",
            "No keyboard-backlight sysfs path is available",
        );
    };
    let Some(target) = device.attributes.get("sysfs_target") else {
        return TestResult::new(
            &req.test,
            Outcome::Blocked,
            "backlight_identity_missing",
            "Keyboard-backlight identity is incomplete",
        );
    };
    let count = req
        .inventory
        .devices
        .iter()
        .filter(|candidate| {
            candidate.kind == Kind::KeyboardBacklight
                && candidate.recognition == Recognition::Detected
        })
        .count();
    if count != 1 {
        return TestResult::new(
            &req.test,
            Outcome::Blocked,
            "backlight_ambiguous",
            "UPower's legacy keyboard-backlight endpoint cannot be mapped when multiple backlights exist",
        );
    }
    let input = CycleRequest {
        sysfs_path: PathBuf::from(path),
        sysfs_target: PathBuf::from(target),
    };
    match run_cycle(&input) {
        Ok(cycle) => {
            let mut result = TestResult::new(
                &req.test,
                Outcome::Passed,
                "backlight_cycle_delivered",
                "Keyboard-backlight levels changed and the original level was restored; visual confirmation is still required",
            );
            result.measure("original_brightness", cycle.original, "level");
            result.measure("test_low_brightness", cycle.low, "level");
            result.measure("test_high_brightness", cycle.high, "level");
            result.measure("restored_brightness", cycle.restored, "level");
            result.measure("max_brightness", cycle.maximum, "level");
            result.evidence.push("UPower control/readback establishes a level change, not whether the keys visibly illuminated".into());
            result
        }
        Err(error) => TestResult::new(
            &req.test,
            Outcome::Blocked,
            "backlight_control_unavailable",
            error,
        ),
    }
}

fn run_cycle(input: &CycleRequest) -> Result<CycleResult, String> {
    let mut command = Command::new(std::env::current_exe().map_err(|error| error.to_string())?);
    command
        .arg("_backlight_cycle")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe extern "C" {
            fn setsid() -> i32;
        }
        unsafe {
            command.pre_exec(|| {
                if setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("Cannot start backlight restorer: {error}"))?;
    let write = child.stdin.take().ok_or("Restorer input unavailable")?;
    let mut write = write;
    if serde_json::to_writer(&mut write, input).is_err() || write.flush().is_err() {
        return Err("Cannot send selected backlight identity to restorer".into());
    }
    drop(write);
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Keyboard-backlight cycle failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(1200)
                .collect::<String>()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Invalid backlight cycle evidence: {error}"))
}

pub fn guardian_main() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("Linux required".into());
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(8192)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let input: CycleRequest = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let result = cycle(&input)?;
    serde_json::to_writer(std::io::stdout().lock(), &result).map_err(|error| error.to_string())?;
    Ok(())
}

fn cycle(input: &CycleRequest) -> Result<CycleResult, String> {
    validate_path(&input.sysfs_path, &input.sysfs_target)?;
    let maximum = read_level(&input.sysfs_path.join("max_brightness"))?;
    let original = read_level(&input.sysfs_path.join("brightness"))?;
    if maximum < 2 || original > maximum || original < 0 {
        return Err("Invalid keyboard-backlight brightness range".into());
    }
    if get("GetMaxBrightness")? != maximum || get("GetBrightness")? != original {
        return Err("UPower endpoint does not match the selected keyboard backlight".into());
    }
    let low = 0;
    let high = maximum;
    // Always attempt restoration after the first write, including if a later
    // UPower call fails. This process survives worker-group cancellation.
    let attempt: Result<(), String> = (|| {
        set(low)?;
        if get("GetBrightness")? != low {
            return Err("Low backlight level was not reached".into());
        }
        thread::sleep(Duration::from_millis(900));
        set(high)?;
        if get("GetBrightness")? != high {
            return Err("High backlight level was not reached".into());
        }
        thread::sleep(Duration::from_millis(900));
        Ok(())
    })();
    let restoration = set(original).and_then(|_| get("GetBrightness"));
    let restored = restoration.map_err(|error| {
        format!("Could not restore original keyboard brightness {original}: {error}")
    })?;
    if restored != original {
        return Err(format!(
            "Keyboard brightness restored to {restored}, expected {original}"
        ));
    }
    attempt?;
    Ok(CycleResult {
        original,
        low,
        high,
        restored,
        maximum,
    })
}

fn validate_path(path: &Path, target: &Path) -> Result<(), String> {
    if path.parent() != Some(Path::new("/sys/class/leds"))
        || !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains("kbd_backlight"))
        || !target.starts_with("/sys/devices/")
        || fs::canonicalize(path).map_err(|error| error.to_string())? != target
    {
        return Err("Selected keyboard-backlight identity changed".into());
    }
    Ok(())
}

fn read_level(path: &Path) -> Result<i32, String> {
    read(path)
        .ok_or_else(|| format!("Cannot read {}", path.display()))?
        .parse::<i32>()
        .map_err(|error| error.to_string())
}

fn get(method: &str) -> Result<i32, String> {
    let output = call(method, &[])?;
    let mut fields = output.split_whitespace();
    if fields.next() != Some("i") {
        return Err(format!("Unexpected UPower {method} response"));
    }
    let value = fields
        .next()
        .ok_or("UPower returned no brightness value")?
        .parse::<i32>()
        .map_err(|error| error.to_string())?;
    if fields.next().is_some() {
        return Err(format!("Unexpected UPower {method} response"));
    }
    Ok(value)
}

fn set(value: i32) -> Result<(), String> {
    call("SetBrightness", &["i", &value.to_string()]).map(|_| ())
}

fn call(method: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("busctl")
        .args([
            "--system",
            "--timeout=2s",
            "call",
            SERVICE,
            OBJECT,
            INTERFACE,
            method,
        ])
        .args(args)
        .output()
        .map_err(|error| format!("UPower control unavailable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "UPower {method} failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(600)
                .collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_an_unrelated_sysfs_target_before_any_control_call() {
        let result = validate_path(
            Path::new("/sys/class/leds/smc::kbd_backlight"),
            Path::new("/tmp/unrelated"),
        );
        assert!(result.is_err());
    }
}
