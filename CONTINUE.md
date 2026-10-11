# omacdiag: implementation handover and remaining delivery plan

Last updated: 2026-09-30.

**Continuation update (Linux `MacBookAir5,1`, same date):** A local Git baseline was created at `eae7897`; the follow-up implementation remains in the working tree. The current Linux source passes 31 Rust tests, strict Clippy, both debug builds, formatting, deterministic schema regeneration, and a 31-event contract check. Refresh, automatic evidence, missing device nodes, selected-device identity, nested cancellation, V4L2 metadata-only nodes, and HDMI playback routing now have focused coverage. Bounded CPU, memory, GPU, display-connector, telemetry, analog audio, and keyboard-backlight control checks ran on the host with limited outcomes; see [the Linux baseline record](docs/linux-baseline-2026-09-30.md). The user's automatic report showed real microphone capture but could not record a listening verdict; guided capture replay, louder stream-local tones, and timing-tolerant acoustic correlation are now implemented. One follow-up acoustic run passed the joint speaker–microphone path, while guided listening quality remains unverified. Keyboard-backlight control now cycles off/bright and restores the starting level using a separate restorer that survived a tested worker cancellation; visual confirmation remains required. The installed Quickshell loaded the frontend and started the backend after a QML syntax fix. UI guards prevent stale plan responses from enabling Start and require explicit screen selection/pattern display before a visual pass; edited QML parses and startup still succeeds, but guided interaction remains unverified. Archive packaging preflight has four passing tests but no pinned runtime or release artifact has been validated. The user's host-level inventory confirms usable `/dev` nodes that the restricted Codex shell hides; camera quality, external audio, packaged desktop runtime, and physical certification remain unverified. The historical handover status below describes the state before this update.

This document is the continuation plan for the next engineer. Read it before changing the implementation. It distinguishes implemented code from verified behaviour and physical certification.

## 1. Objective and current scope

Build a diagnostic application for Intel Macs released between 2009 and 2017. Discover hardware, exercise each available capability, collect automatic evidence and technician observations, and export a structured JSON report. Show the necessary equipment before any functional test starts. Separate discovery, planning, execution, assessment, and reporting.

The current implementation runs diagnostics on **Linux installed or booted on a Mac**. Development and portable checks can run on macOS, but native macOS discovery and hardware probes are not implemented. On a non-Linux host the application returns an unsupported/empty inventory rather than pretending to test hardware. If native macOS execution is required, use the separate platform phase below; do not describe it as existing support.

Required principles:

- Detect, test, and report. Do not repair hardware, install firmware, change drivers, format disks, mount media, alter persistent mixer settings, or reconfigure networks.
- Keep absence, unavailable permissions, missing drivers, missing equipment, and an observed failure distinguishable.
- Report selected-check results honestly; a passing selection is not certification of the entire machine.
- Inform the technician of equipment, permissions, actions, and approximate duration ahead of execution.
- Retain prior evidence when repeating a check, including checks of another physical connector.
- Keep captured audio/images transient. Persist measurements and observations, not recordings.
- Do not infer exact release years or port layouts from ambiguous model identifiers.

### Technology decision already reflected in the code

Rust owns the core, probes, contracts, CLI, and process control. It is suitable for device identity checks, bounded processing, cancellation, typed outcomes, and distributing a compiled CLI. Quickshell/QML owns the Linux technician interface. Python is used only for development schemas, contract checks, and packaging scripts; it is not an application runtime dependency.

Rust minimum version is 1.88; release recipes currently use 1.93.0. Dependencies are Clap, Serde, serde_json, and the JPEG-enabled image crate. Linux EGL is loaded dynamically rather than linked as a mandatory hardware library.

## 2. Current implementation inventory

### Components and ownership

| Files | Responsibility | Current state |
|---|---|---|
| `src/model.rs`, `src/report.rs`, `schemas/` | Typed contracts, evidence, coverage, private atomic JSON persistence | Implemented; newest fields need fresh regression/contract execution |
| `src/discovery.rs` | Linux sysfs/procfs/device enumeration and driver evidence | Implemented; real-machine completeness unverified |
| `src/profiles.rs`, `data/model-profiles.json` | Family expectations and source-backed model connector counts | Implemented for a limited catalogue |
| `src/planner.rs` | Selection, equipment aggregation, readiness, instructions, estimates, prerequisites | Implemented |
| `src/engine.rs`, `src/process.rs` | Sequential workers, deadlines, cancellation, checkpoints, guided assessment | Implemented; adversarial and physical validation remain |
| `src/probes/basic.rs` | CPU, memory, storage, connectors, networking, power and peripheral evidence | Implemented with limits below |
| `src/probes/media.rs` | Camera, microphone, speakers, acoustic correlation, audio jack evidence | Implemented; calibration and routing validation remain |
| `src/probes/gpu.rs` | Per-device EGL/GLES2 readback | Linux implementation present; real Linux GPU validation remains |
| `src/main.rs`, `src/protocol.rs` | CLI and JSON Lines session interface | Implemented |
| `src/bin/helper.rs`, `packaging/org.omacdiag.smart.policy` | Optional narrow read-only SMART privilege helper | Implemented; installed-policy validation remains |
| `ui/Session.qml`, `ui/shell.qml`, `ui/Card.qml`, `ui/Patterns.qml` | Technician workflow, previews, assessment, patterns, export | Implemented; runtime unverified |
| `tests/core.rs`, `tests/profiles.rs`, `examples/contract_fixture.rs` | Portable regression fixtures and synthetic report data | Present; rerun after the latest changes |
| `scripts/`, `packaging/`, `.github/workflows/ci.yml` | Schema generation, contract validation, pinned runtime builds and archives | Recipes present; shipped Linux desktop artifact not established |

### Discovery and planning

The code enumerates CPU/memory, PCI and USB devices, USB root hubs, V4L2 cameras, ALSA capture/playback endpoints, whole block devices, optical drives, DRM connectors/render nodes, network interfaces, Bluetooth, input devices, power/battery, hardware sensors, Thunderbolt, FireWire, and keyboard backlight. It recognizes the Broadcom PCIe camera identity even when no usable capture endpoint exists. Input classification considers key capabilities and trackpad evidence.

Family baselines cover notebooks, iMac, Mac mini, and Mac Pro. Published connector profiles cover seven identifiers: `MacBookPro8,1`, `MacBookPro12,1`, `MacBookPro14,1`, `Macmini6,1`, `Macmini6,2`, `iMac12,2`, and `MacPro5,1`. These are specifications, not physically certified support entries. Source URLs are retained. Other layouts remain explicitly unknown.

The planner builds quick/extended and automatic/guided plans. Equipment readiness is available/unavailable/uncertain. Unknown equipment IDs and selected test IDs are rejected. Equipment requirements are aggregated from selected checks. Missing expected components receive availability checks rather than invented functional passes.

### Functional probes and evidence limits

| Capability | Implemented evidence | Limit / remaining work |
|---|---|---|
| CPU | Deterministic arithmetic, short or sustained workload, thermal stop | Single worker; does not certify all cores or instruction paths |
| Memory | Bounded allocated-memory patterns | Approximately 32 MiB quick / 256 MiB extended maximum; not all physical RAM |
| Storage | SMART/NVMe JSON interpretation and distributed read-only samples | No self-test initiation, repair, full-sector scan, or write test |
| USB / SD | Newly inserted device, controller ancestry, mounted-device identity, exact 64 KiB fixture content | One attempt does not certify every connector; physical labels supplied by operator |
| Optical | Read-only media samples | Media quality and format compatibility affect evidence |
| Camera | Several frames, content/change metrics, live JPEG previews | Dark/static scenes may be inconclusive; visual quality is guided |
| Microphone | Live PCM levels, peak/clipping and aggregate evidence | Must validate silence/noise thresholds and real input routing |
| Speakers | Explicit ALSA playback, channel-specific tones, operator quality assessment | Successful stream delivery alone is inconclusive |
| Acoustic loop | Physical microphone capture correlated with emitted channel sequences | Positive result establishes joint path; negative result does not isolate a component |
| GPU | Exact EGL render-node matching, colour-quadrant readback, software renderer rejection | Basic graphics path only; no exhaustive VRAM/3D certification |
| Displays | Connector state, EDID checksum, selectable fullscreen patterns | Visual outcome and correct connector/screen mapping need physical confirmation |
| Audio jack | Jack-related evidence and guided headset assessment | Connector semantics differ by model; not every jack accepts a microphone |
| Network | Interface-bound numeric-peer ICMP exchange | Does not configure networking or establish throughput; policy can block ICMP |
| Bluetooth | Adapter-specific BlueZ connection evidence plus guided peripheral exercise | Pairing/setup is external; a connected link alone is limited evidence |
| Thunderbolt | New authorized peripheral/link evidence plus operator transfer assessment | USB in the same socket is not a Thunderbolt test |
| FireWire and other physical functions | Guided evidence where a direct probe is unavailable | Automated transfer verification is not universally implemented |
| Battery / sensors | Telemetry; charging transition evidence | Wear/readings do not certify runtime or sensor accuracy |
| Keyboard / pointer / backlight and model-specific functions | Guided exercise and recorded observations; bounded UPower off/bright/restore cycle for a sole keyboard backlight | Full key coverage, gestures, visible backlight change, and model-specific capabilities need validation |

Quick CPU/RAM durations are approximately 2–3 seconds. Extended CPU/RAM last 120 seconds each; extended GPU readback lasts 60 seconds. Extended storage samples up to 128 MiB within a deadline. Estimates exclude unpredictable technician time. Thermal readings at or above 90°C conservatively stop load probes; sensor accuracy itself remains unverified.

### Session, reports, and frontend

- Sequential isolated worker processes prevent conflicting audio/device tests. Deadlines and cancellation terminate worker process groups. Output is bounded. Linux child processes also request termination when their parent dies.
- In guided mode every executed check has a post-run assessment. Checks needing physical setup or probe inputs first have a preparation stage; each retake takes a fresh baseline and preserves earlier results. Final observations preserve supplied setup details.
- Provisional guided passes are checkpointed as incomplete while awaiting assessment.
- Reports are atomically replaced using private `0600` temporary files, syncing before rename. Prior completed evidence survives an interrupted running check.
- Seven result outcomes exist: passed, failed, inconclusive, blocked, skipped, unsupported, cancelled. Overall is passed/failed/incomplete.
- The latest change adds nullable `automatic_outcome` and `automatic_reason_code`, preserving worker evidence independently of the combined result. Manual-only/pre-execution skipped checks have null automatic fields. Operator skip cannot hide automatic failure.
- Protocol version 1 supports preparation, inventory, pre-test refresh, start, guided replies, cancel, repeat, export, and quit. Replies must match the pending test, prompt, and stage.
- The latest `refresh` command is permitted only in a prepared session without results. It rediscovers hardware, retains profile/mode, resets selection to the complete new catalogue, and requires equipment/selection review and renewed acknowledgement. Readiness is supplied by the caller.
- Repeat appends a uniquely identified attempt and retains earlier evidence. Starting again runs only pending attempts.
- QML provides equipment/readiness, inventory, test selection, microphone selection, live camera/microphone feedback, keyboard/pointer exercises, fullscreen display patterns, operator notes, repeat, and JSON export.
- CLI diagnostics currently do not expose a documented distinct exit code for failed versus incomplete reports. Automation should inspect JSON until an exit-code contract is explicitly designed.

## 3. Verification status at handover

Confirmed during the latest implementation turn:

- `cargo build --offline --all-targets` completed before the final small refresh-state guard adjustment.
- Strict host Clippy completed after that guard adjustment.
- Synthetic Linux-conditional Clippy exposed an ambiguous GPU iteration-counter type; the counter was fixed to `u64`, and that check then completed.
- Rust formatting ran and JSON Schemas were regenerated for the latest fields/events.

Earlier portable regressions and schema/session checks had successful runs. **Do not treat those earlier runs as verification of the latest automatic-evidence and refresh changes.** The full current suite and updated contract scenarios must be run next. The contract validator presently does not exercise `refresh`.

Not established:

- A real Linux build/run result for the current final source.
- Quickshell/QML parsing and runtime behaviour using the pinned runtime.
- Physical hardware probe results or acoustic calibration.
- A certified model, distribution/driver matrix, installed helper policy, or distributable desktop runtime.
- Remote CI success; workflow files are present, but no remote results are asserted.

The current workspace was created without Git metadata. Check whether a repository has since been initialized before relying on Git status/history or opening a PR.

## 4. Logical continuation phases

The phases below are ordered by dependency. Software checks can proceed before a hardware kit exists. Prepare the kit and target machines in parallel. Release requires the software, desktop, physical, and packaging gates to pass.

### Phase A — Establish the baseline and lock the delivery scope

**Owner:** core engineer / project lead.

Tasks:

1. Read this file, README, `docs/architecture.md`, `docs/protocol.md`, `docs/validation.md`, and `docs/packaging.md`.
2. Inventory source and build artifacts. Initialize version control if absent and appropriate; capture a baseline commit before further changes.
3. Record toolchain versions and the source revision for every following validation run. Keep the existing lockfile.
4. Confirm Linux-on-Mac is the first release scope. If native macOS is required, track that explicitly as Phase J.
5. Define initial supported models/distributions and whether the first deliverable is CLI, desktop archive, installed application, or live image. Existing code supports CLI/desktop archive recipes; native packages and a live image are separate additions.
6. Specify production acceptance rules: selected checks versus a refurbishment checklist, permitted gaps, required operator details, and evidence retention.
7. Decide a report compatibility policy before external consumers adopt schema v1. Newly required nullable fields have been added during development; old reports may need migration if they already exist outside this workspace.

**Deliverable:** reproducible baseline and a written initial-release support scope.

**Exit criterion:** next engineers and technicians know which OS, models, checks, and deliverable are being validated.

### Phase B — Complete core regression and contract coverage

**Owner:** Rust/core engineer. Depends on A.

Tasks:

1. Add outcome-combination tests for `engine::assessed_outcome`: automatic failure plus operator skip/pass/inconclusive; blocked/unsupported/cancelled plus pass; provisional pass plus operator failure/inconclusive; manual-only assessment; and preservation of automatic fields after combined reasons change.
2. Verify provisional checkpoint reports keep automatic evidence while their combined outcome remains incomplete pending assessment.
3. Extend `scripts/validate_contracts.py` to send refresh before an empty-selection run, validate the refreshed inventory/plan payload, and confirm profile/mode retention, selection reset, readiness, and checkpoint persistence.
4. Reject refresh during execution and after a completed/cancelled run, including a completed empty selection. Ensure rejection leaves reports untouched.
5. Exercise unknown equipment IDs, unknown selections, malformed JSON, unsupported versions, duplicate start, stale prompt replies, and repeat state transitions.
6. Ensure the synthetic report fixture is built; the validator otherwise skips it when missing. Include non-null automatic evidence as well as null fields and every outcome.
7. Review schema constraints against Serde defaults/nullable fields. Regenerate schemas and check that regeneration is deterministic and leaves no unexplained differences.
8. Review combined-outcome reason consistency. Automatic evidence must remain traceable even when an operator-observed failure changes the final reason.
9. Audit hard interruption behaviour: SIGINT/SIGTERM, controller death, worker death, nested tool death, report-directory failure, full filesystem, and bounded output. Add portable fixtures where possible.
10. Review preflight and no-probe commands: inventory, equipment, plan, prepare, and refresh must not capture, play audio, stress hardware, or initiate functional device operations.
11. Review physical coverage accounting. Current published-count comparisons deduplicate USB labels; they do not certify every SD/audio/display/FireWire/Thunderbolt connector or reject arbitrary labels as a verified layout. Decide and document stronger accounting before full-machine certification claims.

Run once coverage is updated:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --all-targets
cargo build --locked --examples
uv run scripts/validate_contracts.py
```

Use `--offline` where dependencies are already cached. A synthetic Linux cfg check on macOS is only an additional static check, never the Linux gate.

**Deliverable:** current-source regression and schema/session results with logs.

**Exit criterion:** all checks pass, newest behaviour is exercised, and report compatibility/coverage semantics are explicit.

### Phase C — Establish real Linux execution and validate identity/prerequisites

**Owner:** Linux/core engineer. Depends on B; can begin with a VM for process/tool behaviour.

Tasks:

1. Build on real x86-64 Linux using baseline CPU features, initially the oldest intended userspace. Do not use `target-cpu=native` or require AVX on older Macs.
2. Run portable regressions and contract checks on Linux. Then run the distribution matrix in CI or equivalent reproducible environments; fix container/toolchain/image assumptions as needed.
3. Inventory at least one physical Mac and compare with independent read-only Linux tools. Record PCI/USB identities, driver bindings, ALSA cards/PCM endpoints, DRM connectors/render nodes, block ancestry, and DMI identity.
4. Validate missing/busy/permission-denied prerequisites, especially cameras, ALSA devices, render nodes, block reads, SMART, BlueZ, and absent executable dependencies.
5. Recheck device identity immediately before operations where endpoint names can be reused after hotplug. Add canonical sysfs/device identity evidence and reject reassignment where necessary. Check PCI camera endpoint ancestry and GPU render-node-to-PCI correspondence.
6. Verify process-group cancellation and parent-death behaviour on Linux, including threaded spawning and capture programs. Confirm no orphan playback/recording/load process remains.
7. Validate readable fixture ownership against USB/SD ancestry with partitions, hot unplug, unrelated/system filesystems, adapters, and multiple attached media.
8. Review optional helper installation: only whole block devices, canonical identity match, absolute executable, cleared environment, correct policy path, no shell command expansion, denial reporting, and no broader privileged operation.
9. Preserve all missing-driver evidence. Environment setup must remain outside the diagnostic application.

**Deliverable:** genuine Linux build/run logs and a device identity/prerequisite audit.

**Exit criterion:** probes act on the selected device and fail conservatively under missing prerequisites or reassignment.

### Phase D — Validate and refine the Quickshell workflow

**Owner:** frontend engineer. Depends on B and a real Linux Qt/Quickshell environment.

Tasks:

1. Obtain/build matching Quickshell 0.3.1 and Qt 6.8.3 as described in packaging docs. Run available QML lint/compile tooling and launch the actual UI.
2. Resolve imports, process APIs, bindings, control sizing, window lifecycle, and runtime warnings. No successful QML runtime check is currently recorded.
3. Confirm the checklist is visible before functional execution. Readiness/selection changes must produce the latest plan before Start is enabled.
4. Validate refresh after attaching a supporting microphone/display: catalogue updates, selections reset visibly, profile/mode/readiness remain coherent, acknowledgement resets, and errors recover without an incorrect ready state.
5. Confirm microphone selection identifies a real physical capture endpoint; reject software loopback and preserve explicit selection when appropriate.
6. Exercise setup/assess prompts, details, optional notes, Pass/Fail/Inconclusive/Skip/Retake, cancellation, and repeated connector attempts. Ensure evidence cards distinguish provisional results from completed assessment.
7. Verify live JPEG preview and PCM updates without unbounded buffering or retaining media in reports.
8. Validate display-pattern placement against the intended physical connector. Quickshell screen ordering is not inherently a DRM identity mapping; implement mapping or require explicit operator identification and record it.
9. Test fullscreen behaviour, Escape, focus, DPI scaling, multiple monitors, hot unplug, and zero/changed screen lists under X11 and Wayland.
10. Review keyboard/pointer exercise completeness, focus capture, special keys, gestures, and ability to leave the test area. Do not interpret a few captured events as full-device certification.
11. Exercise slow/failed backend replies, stale responses, engine exit, window close during a test, cancellation while awaiting an operator, export errors, and report-path readability.
12. Verify software-rendered UI on old/failing graphics while the GPU worker independently exercises the selected GPU.

**Deliverable:** working technician walkthrough, resolved runtime defects, and frontend validation notes.

**Exit criterion:** the complete guided workflow works without stale-plan starts, wrong-screen assessment, or lost checkpoints.

### Phase E — Prepare and document the physical test bench

**Owner:** validation lead / technician. Can proceed alongside B–D.

Prepare equipment before scheduling a session:

| Equipment | Preparation and purpose |
|---|---|
| Known-good USB media | Create the 64 KiB fixture in advance; provide USB-A/USB-C connections; label the device and adapters |
| Known-good SD card | Compatible card with the fixture; test the actual built-in reader rather than bypassing it |
| Known-good headset | Identify supported headphone/microphone wiring for each model; distinguish output-only and input connectors |
| Physical external microphone | Required where usable built-in capture is absent; confirm capture and avoid software-monitor inputs |
| External monitor | Powered display, known-good compatible cables/adapters for HDMI, Mini DisplayPort, Thunderbolt, USB-C as applicable |
| Power supply | Correct known-good charger/cable and wattage; desk Macs need compatible mains power |
| Network peer | Known reachable numeric address, Ethernet cable or prepared Wi-Fi network; ICMP policy known |
| Bluetooth peripheral | Pair/connect through normal desktop tools; define an actual peripheral function to exercise |
| Optical disc | Known readable media matching the drive |
| FireWire device | Genuine peripheral/cable and a defined read/transfer exercise |
| Thunderbolt device | Genuine compatible peripheral/cable; define transfer evidence separate from USB |
| Validation records | Machine ID, model, OS/kernel, firmware, driver versions, kit IDs, environment and technician identity |

Create fixtures only by explicit preparation:

```sh
omacdiag fixture /mounted/media/omacdiag-fixture.bin
```

This creates a new file and refuses overwrite. Diagnostics themselves only read it. Mounting, pairing, network setup, driver setup, and external display configuration are technician/environment tasks.

Write a bench guide distinguishing equipment attached before inventory refresh from insertion-test fixtures that must be removed before the baseline prompt. Label each physical port consistently. Document acoustic room conditions and monitor placement. Preserve privacy by avoiding real user media and recordings.

**Deliverable:** equipment checklist, labelled known-good fixtures, and a repeatable technician procedure.

**Exit criterion:** no functional test is scheduled without its required equipment or an explicit planned gap.

### Phase F — Pilot physical Macs and calibrate probes

**Owner:** validation lead with core/frontend engineers. Depends on C–E.

Start with an older representative Mac and a newer representative Mac; include both legacy USB/display connectors and USB-C/Thunderbolt where available. Run inventory first, then quick automatic, quick guided, repeats for every physical connector, and extended checks after quick behaviour is understood.

Tasks by subsystem:

1. **Discovery:** reconcile every expected component with actual hardware, driver state, and usable endpoints. Missing capture drivers must not become defective-camera declarations.
2. **USB/SD:** exercise every physical port/reader, correct and incorrect fixture paths, no insertion, disconnected media, multiple media, and native versus adapter paths. Preserve distinct labels and prior attempts.
3. **Camera:** illuminated changing scene, dark/static scene, no frames, busy device, correct preview identity, and visible quality defects.
4. **Microphone:** normal speech/test stimulus, silence, noisy room, clipping, mute/busy input, built-in and external inputs. Calibrate measurements against known-good behaviour.
5. **Speakers/acoustic:** both channels, actual output route, mute/low volume, headphones/HDMI routing, ambient noise, clipping, software loopback rejection, and independently disconnected signal paths. Tune correlation/alignment/signal thresholds using reproducible data; keep negative joint-path results inconclusive when the faulty component cannot be isolated.
6. **Audio jacks:** insertion evidence and actual output/input function for the model's supported connectors. Record the selected physical jack and headset compatibility.
7. **GPU:** each discrete/integrated GPU and render node, hybrid/multi-GPU systems, software rendering, unavailable EGL mapping, thermal interruption, and bad readback fixtures. A basic pass must remain labelled as a basic test.
8. **Displays:** internal/external panel identity, EDID, cable/adapter combinations, correct fullscreen placement, colour/gradient/grid assessment, disconnects, and visible defects.
9. **CPU/memory:** short and extended behaviour, thermal stop, bounded allocation, cancellation and coexistence with UI. Document that passing samples do not certify all hardware.
10. **Storage/optical:** healthy/unhealthy SMART fixtures, unreadable/permission-denied drives, sampled reads, missing smartctl, and helper denial. Verify no disk writes or SMART configuration changes occur.
11. **Power/battery/sensors:** charger state transitions, desktops versus notebooks, battery wear, unavailable sensors, and extreme/implausible readings. Avoid battery-runtime or sensor-accuracy claims.
12. **Network/Bluetooth:** correct selected interface/adapter, peer unavailable, policy-blocked ICMP, connected peripheral function, and external setup dependency.
13. **Thunderbolt/FireWire:** actual protocol peripheral, physical connector labels, insertion/link evidence, and actual read/transfer exercise. Link presence alone is insufficient for broad certification.
14. **Input and other functions:** full relevant keyboard/pointer exercise, keyboard lighting, Touch Bar where exposed, and optical/model-specific guided tasks.
15. **Resilience:** unplug during execution, cancel while prompting/capturing/loading, worker crash, UI close, controller crash, and interrupted report writes. Inspect partial JSON and surviving processes.

Store a redacted JSON report, software revision, kit identity, steps, expected/actual outcomes, screenshots where useful, and defect references for every scenario. Record calibration data without adding persistent media capture to the production report path.

**Deliverable:** pilot evidence, calibrated thresholds, and fixes driven by real-machine results.

**Exit criterion:** known-good machines behave reproducibly; failure/absence/prerequisite scenarios remain distinguishable; unresolved defects are documented rather than hidden by passes.

### Phase G — Expand the model catalogue and certify the target matrix

**Owner:** hardware validation engineer. Depends on F.

1. Expand from the seven identifiers to the intended 2009–2017 families: MacBook/MacBook Air/MacBook Pro, iMac, Mac mini, Mac Pro, and separately evaluate the 2017 iMac Pro if included.
2. Source exact connector counts and capabilities from authoritative model specifications. Keep source URLs and versioned profile identifiers. Do not resolve reused model identifiers to a fabricated exact year.
3. Add physical port labels/diagrams or a validated label vocabulary where reliable. Shared USB-C/Thunderbolt/display/power capabilities should reference one connector with multiple independent functions rather than appear as separate sockets.
4. Track per-connector coverage for functions beyond USB if full refurbishment completeness is required. Distinguish not tested, missing kit, unavailable backend, failed exercise, and expected absence.
5. Add profile regression fixtures for new expectations and prevent false SD/optical/jack assumptions on models without them.
6. Build a model × kernel/distribution × driver/firmware × capability matrix. Preserve driver limitations separately from hardware defects.
7. Execute X11/Wayland validation across declared supported distributions. Containers validate userspace builds, not physical graphics/audio/device support.
8. Classify each model as validated, partially validated, blocked by environment, or unverified. No family-wide certification based on a single machine.

**Deliverable:** source-backed catalogue and an evidence-backed support matrix.

**Exit criterion:** declared supported models have sufficient real reports; every unsupported or incomplete capability is explicit.

### Phase H — Build and verify release artifacts

**Owner:** release engineer. Depends on C/D; final release depends on F/G for declared models.

1. Reproduce release binaries on the oldest supported userspace using baseline x86-64 features and locked dependencies.
2. Build the pinned Qt/Quickshell runtime with `scripts/build_runtime.py`. Verify the clean tagged source, matching private Qt APIs, plugins/QML imports, XCB/Wayland support, and manifest.
3. Package CLI and desktop archives with `scripts/package.py`; use fresh staging directories. Ensure every included path and launcher works outside the source tree.
4. Verify shared-library resolution on each target distribution. Exercise the bundled runtime without relying on an accidental development SDK or system Qt match.
5. Test staging/install layouts, `/opt/omacdiag`, launcher paths, desktop entry, helper location, Polkit policy, normal user execution, denial behaviour, and report directory permissions.
6. Audit third-party license/source materials for the actual distributed Qt/Quickshell SDK and tools. Current copying scripts alone are not a completed redistribution audit.
7. Generate checksums, provenance, version/source information, and signed artifacts if the release process requires them.
8. Document required external tools (`ffmpeg`, ALSA utilities, smartctl, ping, BlueZ/busctl, EGL/GLES) and any unbundled distribution dependencies.
9. Decide separately whether native `.deb`/`.rpm` packages or a bootable live environment are needed. They are not generated by the existing archive recipes; do not block an agreed archive release on optional formats.
10. Install on a clean target system and execute the full technician workflow, report export, cancellation, and helper checks.

**Deliverable:** reproducible release artifacts and installation/runtime evidence.

**Exit criterion:** clean supported systems run the declared capabilities with documented dependencies and correct privileges.

### Phase I — Production readiness and maintenance handoff

**Owner:** project lead with engineering/validation owners. Depends on B–H.

1. Update README and validation docs with actual results, supported scope, unresolved limitations, and verified versions. Remove development caveats only when their evidence exists.
2. Publish the equipment guide before the download/start instructions. Include timings, driver prerequisites, physical labels, and examples of incomplete reports.
3. Provide sample redacted reports and consumer guidance for null automatic evidence, observations, repeat attempts, coverage gaps, and selected-check overall status.
4. Specify report migrations, API/version changes, and CLI exit-code semantics if required by downstream automation.
5. Define acceptance and escalation procedures for ambiguous evidence. Report symptoms and tested operations; do not prescribe hardware replacement from a single failed probe.
6. Establish regression fixtures for discovered defects, periodic dependency/runtime rebuilds, catalogue updates, and repeat physical validation on representative machines.
7. Document report retention/export/privacy policy and responsibility for external technician setup/restoration.
8. Review the final release gate with evidence for each supported claim and list remaining unsupported models/functions.

**Deliverable:** usable release documentation, support ownership, and a defensible release decision.

**Exit criterion:** another engineer and technician can reproduce the build and the validated inspection procedure without session history.

### Phase J — Native macOS backend, only if required

This is additional scope, not an existing implemented capability or a prerequisite for an agreed Linux-first release.

1. Define supported macOS versions across the 2009–2017 hardware range before choosing APIs/toolchain and distribution format.
2. Keep Rust contracts/planner/reporting and add a platform abstraction for discovery and execution. Avoid embedding macOS decisions in Linux probe code.
3. Implement macOS inventory and device identity using appropriate system APIs; design camera/microphone permission handling and transient capture/playback adapters.
4. Replace Linux sysfs, ALSA, V4L2, EGL-device, BlueZ and block-path assumptions with platform equivalents while retaining evidence semantics.
5. Choose a macOS-compatible frontend. The current Quickshell Linux workflow is not a native macOS delivery strategy; evaluate a native UI or another portable frontend against required OS versions.
6. Validate sandbox/permissions, privacy descriptions, signing/distribution requirements, helper scope, and cancellation for each supported OS.
7. Run the same physical acceptance matrix and label OS-specific capabilities and limits explicitly in JSON.

**Deliverable:** separately validated macOS application/backend with the shared report contract.

## 5. Recommended first work session

1. Establish a baseline revision and inspect the latest outcome/refresh changes.
2. Add focused regression and contract coverage for those changes.
3. Run the complete current software check sequence and save logs.
4. Move to real Linux and start the pinned frontend runtime while the technician prepares the kit.
5. Fix runtime/identity defects before beginning the physical pilot.

Do not expand the probe set before establishing correctness of the existing report semantics, device selection, and technician workflow. Track optional certification enhancements separately from defects preventing the current intended checks from working.

## 6. Release completion checklist

- [ ] Current-source Rust regression and schema/session checks pass.
- [ ] Refresh and independent automatic evidence have explicit regression coverage.
- [ ] Real Linux build and process/device prerequisite checks pass.
- [ ] Quickshell workflow runs on pinned Qt under X11 and Wayland.
- [ ] Equipment guide and labelled known-good kit are ready before sessions.
- [ ] Physical pilot and acoustic calibration have saved evidence.
- [ ] Device/connector identity and repeat coverage are validated.
- [ ] Every declared supported model/capability has an evidence-backed status.
- [ ] Clean-system archive/runtime/install/helper validation passes.
- [ ] Third-party redistribution materials and artifact provenance are complete.
- [ ] Documentation clearly separates selected-check passes from full-machine certification.
- [ ] Native macOS scope is explicitly implemented/validated or explicitly outside this release.

Until these gates are met, describe the project as a development implementation with unverified physical/platform coverage.
