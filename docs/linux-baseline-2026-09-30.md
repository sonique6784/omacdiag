# Linux development baseline: MacBookAir5,1

Date: 2026-09-30. Baseline source commit: `eae7897` (imported source). The implementation changes described here remain in the working tree for review; build artifacts and the private machine inventory are ignored by Git. Toolchain: Rust/Cargo 1.98.1, Linux 7.2.5-3-omarchy, x86-64, Intel Core i7-3667U. This Omarchy installation is a development host, not one of the declared release distributions.

## Software checks

Run from the project root using the current source:

```sh
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test --locked --offline
cargo build --locked --offline --all-targets
cargo build --locked --offline --examples
UV_CACHE_DIR=/tmp/omacdiag-uv-cache uv run --offline scripts/validate_contracts.py
python3 scripts/generate_schemas.py
git diff --exit-code -- schemas
```

The first Linux baseline had 18 passing Rust tests, a passing 10-event contract check, and one Rust 1.98 Clippy lint. After the changes, all 38 Rust tests pass, strict Clippy and the example build pass, and the contract validator accepts all five schemas and 39 session events. Formatting and `git diff --check` pass. Python's pinned `jsonschema` dependency and locked Rust crates were downloaded for these checks; neither is an application runtime dependency.

## Read-only machine inventory

The application and `/sys/class/dmi/id/product_name` both identify `MacBookAir5,1`. Independent `lspci -nnk` output confirms Intel HD Graphics 4000 at `0000:00:02.0` with `i915`, Intel USB controllers, Intel HDA audio, Broadcom BCM43224 Wi-Fi, and Intel DSL3510 Thunderbolt. `lsblk` shows a SATA Kingston disk at `sda`. Sysfs exposes two FaceTime V4L2 nodes, one microphone PCM, four playback PCMs, and one DRM render entry. A family baseline applies; exact physical connector counts remain unknown.

The restricted Codex shell exposes sysfs but hides `/dev/video*`, `/dev/dri`, `/dev/snd`, and `/dev/sda`; its inventory correctly records unavailable device nodes. The user's inventory from a normal host terminal confirms those nodes exist. A read-only V4L2 capability query identified `/dev/video0` as video capture and `/dev/video1` as metadata-only. The rebuilt host-level inventory now lists one camera capture endpoint and excludes the metadata sibling. `smartctl` and `lsusb` are absent; `ffmpeg`, ALSA utilities, `ping`, `busctl`, and `v4l2-ctl` are installed. The initial inventory phase ran no camera frame capture, audio playback/capture, storage read, network exchange, or extended load; later bounded audio checks are documented below. A physical technician session needs known-good equipment before assigning physical hardware verdicts.

The worker now rechecks the selected sysfs target, endpoint, driver, and stable identifying attributes immediately before a functional probe. Hotplug races after that check, indistinguishable devices with missing serials, and physical connector identity still require bench validation. The automatic endpoint check does not turn an unavailable node into a hardware failure.

A quick automatic run selected only `cpu:system/compute` and `memory:system/patterns` after reviewing their five-second preflight and zero equipment requirements. Both passed (2.0 and 3.1 seconds respectively), automatic evidence matched the final outcomes, the report was private `0600`, and no `omacdiag` process remained. The report at `/tmp/omacdiag-mba51-core-quick.json` contains machine identifiers and is not part of the repository. This is a pass for two bounded checks, not whole-machine certification.

After broadening the technician flow, a Guided-mode CPU run (a normally automatic check) produced one provisional checkpoint and then an assessment prompt; a Pass with an empty note completed it. A second guided CPU run chose Retake after the first computation, ran the check once more, and then accepted Pass. The final report retained two separately identified results, with only the latest attempt required. Private reports are `/tmp/omacdiag-guided-cpu-flow.json` and `/tmp/omacdiag-guided-cpu-retake.json`. This verifies the generalized CLI flow, not physical interactions in the Quickshell UI.

A separate two-second automatic GPU check passed basic colour readback on `/dev/dri/renderD128`, reporting Mesa Intel HD Graphics 4000. Its `0600` report is `/tmp/omacdiag-mba51-gpu-quick.json`; no diagnostic process remained. This does not certify all GPU memory, display output, or sustained graphics operation.

A separate seven-check read-only telemetry run passed the built-in eDP connector/EDID check. Battery health and four thermal readings remained inconclusive by design because telemetry alone cannot establish accuracy or runtime. The power supply check was inconclusive because it reported offline during this run; this is not a charger-failure verdict. The `0600` report is `/tmp/omacdiag-mba51-telemetry-quick.json`, and no worker remained.

ALSA's read-only device list and `/proc/asound` identify one CS4206 analog playback route, three HDMI playback routes, and one analog capture route. Discovery now names those endpoints and the planner requires an audio-capable external display for HDMI audio assessment. It schedules acoustic speaker checks only for the analog output on this machine. The later audio follow-up below exercised capture and playback, but not guided listening quality or external-display routes.

## Audio follow-up from the technician report

The technician's automatic report `1790741329590-49351-0.json` had a three-second microphone capture pass (`-50.6 dBFS` RMS), an inconclusive analog playback-stream result, and inconclusive acoustic results. The guided microphone-response and speaker-quality checks were skipped because Automatic mode cannot record a human listening verdict. The technician reported hearing tones, which is useful observation but was not entered as a guided assessment in that report. External-display audio and headset checks were skipped for missing equipment.

On this Mac, read-only mixer inspection showed Master, Speaker, and PCM already at 100% before testing. The implementation therefore raises only the bounded PCM test signal from 0.15 to 0.30 full scale (about 6 dB), leaving all mixer settings untouched. Read-only checks after testing confirmed the three controls still at 100%. A guided microphone-response check now replays its transient three-second capture once through an unambiguous analog output; audio remains in memory and is not saved. A smoke run delivered the capture and replay through `plughw:CARD=PCH,DEV=0`, but its human observation was intentionally marked inconclusive because no one listened and assessed it during that run.

The first louder acoustic recheck still remained inconclusive: strong signal above baseline but poor right-channel waveform correlation. The correlator now permits at most 25 ms of independent timing correction per channel while retaining the global match and original thresholds. A subsequent targeted recheck passed the joint speaker–microphone path with correlation 0.443, left 0.786, right 0.688, signal 29.2 dB above baseline, and no clipping. The private measurement-only reports are `/tmp/omacdiag-mba51-acoustic-recheck.json`, `/tmp/omacdiag-mba51-mic-replay-smoke.json`, and `/tmp/omacdiag-mba51-acoustic-timing-recheck.json`. These do not replace a guided listening-quality verdict or calibration on other machines.

## Keyboard-backlight follow-up

The detected `smc::kbd_backlight` LED has a 0–255 range. [UPower's keyboard-backlight interface](https://upower.freedesktop.org/docs/KbdBacklight.html) exposes brightness read/write methods; on this Mac its legacy object matched the sole discovered keyboard LED by current and maximum levels. In guided mode checks requiring technician observation offer Pass, Fail, Inconclusive, Skip, and Retake after one probe run; automatic checks retain their probe verdict without a confirmation prompt. Checks requiring physical setup or inputs retain a clearly labelled pre-run setup step; retakes repeat that setup against a fresh baseline. The technician's note is optional, and the previous exercise checkbox has been removed. A separate bounded restorer turns the keyboard light off, then fully bright, and restores the original level. The application records control readbacks but only an operator's explicit visual confirmation can produce a final pass. Multiple discovered keyboard LEDs or an unavailable UPower control block the automated check rather than selecting an ambiguous device.

A controlled smoke run read original 81, low 0, high 255, restored 81; the automatic control outcome passed and the unobserved human verdict was intentionally inconclusive. A second run cancelled while the level was 0; the detached restorer returned it to 81 and the report marked the check cancelled. Reports are private at `/tmp/omacdiag-mba51-backlight-smoke.json` and `/tmp/omacdiag-mba51-backlight-cancel.json`. Cancellation restoration was verified here; power loss or independent termination of the restorer cannot be guaranteed.

## Desktop startup smoke test

The installed Arch Quickshell 0.3.1 with Qt 6.11.2 initially rejected `ui/shell.qml` at a malformed inline mouse handler; that syntax was fixed. A second 15-second launch loaded the configuration and started the backend. Its private checkpoint remained `prepared`, with zero results. The frontend was then guarded so stale plan/refresh replies cannot enable Start, a rejected start clears its optimistic running state, and a late start acknowledgement cannot reopen a finished session. Display-pattern checks now require an explicit screen selection and pattern-show action before the UI permits an observed pass; selecting a different screen invalidates the prior show action. Both edited QML files parse with `qmlformat`, and earlier 15-second launches loaded the configuration. At that point, the latest technician-flow change parsed but had not yet had a new live GUI smoke test. A nonfatal host portal registration warning appeared because another Quickshell instance was already running. This verifies startup on the installed runtime only; the pinned Qt 6.8.3 bundle, guided UI interaction, connector-to-screen placement, and X11 remain untested.

## Camera follow-up

V4L2 mode enumeration reports 1280×720 MJPEG at 29.97 FPS as the largest and fastest advertised capture mode for `/dev/video0`; the 720p YUYV mode advertises only 10 FPS. The bounded camera probe reported both maximum advertised and verified capture resolution as 1280×720. A guided run delivered about 14.9 FPS, below the technician's 90% threshold, and the final report remained inconclusive with reason `capture_fps_below_advertised`. A separate four-second FFmpeg capture without application frame processing also delivered about 15 FPS. An explicit guided Pass observation could not upgrade the below-threshold automatic result. The live assessment preview used a 320×240 YUYV source and delivered frames throughout the prompt. Separate retake and cancellation runs stopped the active preview and preserved the appropriate report state. Images were not saved. The updated QML configuration loaded on installed Quickshell, but the live preview has not been visually assessed inside the GUI or validated on other camera models.

## Packaging preflight

The archive script now checks both executable inputs, the pinned runtime manifest and essential runtime paths before creating a staging directory. It also refuses to overwrite an existing stage or archive. Four Python packaging tests pass, including a CLI archive made with dummy executables; this validates packaging control flow, not a distributable binary. The pinned Qt 6.8.3/Quickshell 0.3.1 runtime has not been built or packaged on this host, and no target-distribution install has been tested.
