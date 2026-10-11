# Original project plan: omacdiag

This document reconstructs the original project scope and phase structure from the conversation and architecture documentation. It is not a verbatim transcript of an earlier planning response. Implementation status and remaining work belong in [CONTINUE.md](CONTINUE.md).

## Objective

Inspect Intel Macs released between 2009 and 2017. Discover recognized hardware, test its available functions, and produce a JSON report. Cover notebooks and desktop Mac families, with a complete refurbishment workflow rather than only a hardware inventory.

Detect, test, and report only. Do not repair hardware, install drivers or firmware, format disks, or change persistent system configuration.

The initial application targets Linux running on these Macs. Native macOS diagnostics require a separate backend and frontend delivery decision; development on macOS does not establish native diagnostic support.

## Technology and architecture

Use Rust for discovery, planning, diagnostic execution, process control, and typed JSON reporting. Use Quickshell/QML for the Linux technician interface. Development scripts may use Python without introducing a Python dependency into the application runtime.

Keep responsibilities separate:

| Subsystem | Responsibility |
|---|---|
| Contracts and reporting | Device identities, outcomes, measurements, observations, JSON schemas, persistence |
| Discovery and model profiles | Enumerate hardware, identify drivers and capabilities, describe model expectations |
| Planner and equipment preflight | Select checks, aggregate required equipment, explain actions and prerequisites |
| Execution engine | Run isolated checks, enforce deadlines, handle cancellation and checkpoints |
| Hardware probes | Exercise a capability and return evidence |
| Session protocol | Exchange versioned requests, progress, previews, and operator responses |
| Technician interface | Present preparation, inventory, guided actions, visual assessment, and reports |
| Validation and packaging | Establish compatibility, reproduce builds, distribute the application |

## Required workflow

1. Show the general equipment checklist before functional testing.
2. Discover hardware and identify the machine/model where possible.
3. Build a machine-specific plan and explain equipment, permissions, actions, and duration.
4. Let the technician select checks, choose quick or extended testing, and declare equipment readiness.
5. Require explicit preflight acknowledgement.
6. Execute automatic checks and guided physical exercises.
7. Collect measurements and technician observations independently.
8. Preserve completed evidence during interruption and allow independently recorded repeat attempts.
9. Export a JSON report describing results, limitations, and coverage gaps.

Automatic mode must not wait indefinitely for technician input. Guided mode should support preparation before a physical action and assessment after evidence is collected.

## Phase 1 — Contracts and reporting

Define versioned contracts before building probes:

- Machine and device identity, recognition state, driver evidence, and capabilities.
- Test specifications, selected plans, equipment requirements, prerequisites, and estimates.
- Measurements with units, automatic evidence, operator observations, and repeat attempts.
- Outcomes: passed, failed, inconclusive, blocked, skipped, unsupported, and cancelled.
- Overall selected-check status and explicit coverage gaps.
- JSON Schemas and a versioned local session protocol.
- Private atomic report checkpoints and final export.

An empty selection or unfinished required assessment must not produce an overall pass. A passing selected suite must not imply that all hardware or every physical connector was certified.

**Deliverable:** shared contracts, schemas, report generation, persistence, and fixtures.

## Phase 2 — Discovery and model expectations

Build read-only Linux enumeration for:

- CPU, memory, storage, and optical drives.
- USB devices/controllers and SD readers.
- Cameras, microphones, playback devices, and audio-jack evidence.
- GPUs, internal panels, connected external displays, and display connectors.
- Network interfaces, Bluetooth, Thunderbolt, and FireWire.
- Keyboard, pointer/trackpad, keyboard lighting, and model-specific devices where exposed.
- Power supplies, batteries, charging state, and sensors.

Maintain model profiles separately from enumeration. Describe expected components and connector layouts using verified specifications. Keep unknown layouts explicit and distinguish an expected component without a driver from a proven hardware defect.

**Deliverable:** inventory JSON, capability mapping, driver evidence, and an extensible model catalogue.

## Phase 3 — Equipment preflight

Notify the technician of required equipment before execution. Provide a general checklist followed by requirements derived from the machine and selected tests.

| Equipment | Purpose |
|---|---|
| Known-good USB media and adapters | Exercise each USB connector using readable reference content |
| Compatible known-good SD card | Test the built-in card reader |
| Compatible headset | Check supported headphone and microphone jack functions |
| Physical external microphone | Verify acoustic output where built-in capture is unavailable |
| Powered external monitor and cables/adapters | Test display transport and visual output |
| Correct charger or mains cable | Verify power and charging behaviour |
| Prepared network and reachable peer | Exercise Ethernet/Wi-Fi communication |
| Known-good Bluetooth peripheral | Verify connection and actual peripheral function |
| Known-readable optical disc | Exercise an optical drive where present |
| Genuine FireWire peripheral and cable | Exercise FireWire where present |
| Genuine Thunderbolt peripheral and cable | Verify Thunderbolt separately from USB |

Allow available/unavailable/uncertain readiness. Missing equipment creates explicit skipped checks or gaps rather than hardware failures. Prepare reference media ahead of the session. Tell the technician when insertion fixtures must remain disconnected until the action prompt.

**Deliverable:** preparation guide, per-test equipment aggregation, selection controls, and acknowledgement gate.

## Phase 4 — Execution engine and hardware diagnostics

Run independent probes with bounded resource use, deadlines, cancellation, and partial-report checkpoints. Isolate workers so a failure does not terminate unrelated checks. Sequence operations that would interfere with one another, especially audio tests.

Implement quick and extended profiles with clearly stated evidence limits:

- CPU calculations and bounded memory patterns.
- Read-only storage health and sampled reads.
- USB/SD insertion, device identity, and reference-content verification.
- Camera capture, preview, and image-content evidence.
- Microphone capture, signal level, clipping, and guided assessment.
- Speaker playback and channel-specific operator checks.
- Acoustic correlation using a physical microphone to verify the joint speaker-to-microphone path.
- Per-GPU rendering/readback with software-renderer detection.
- Display connector/EDID checks and visual patterns.
- Compatible headset/jack exercises.
- Network-peer exchange and Bluetooth peripheral function.
- Thunderbolt/FireWire and other model-specific guided exercises.
- Charging transitions, battery telemetry, and sensor readings.
- Keyboard, trackpad/pointer, and lighting exercises.

Do not interpret successful enumeration or stream delivery as proof of physical function. Acoustic success can establish a working joint path; negative correlation alone cannot identify which component is faulty. Sampled memory/storage/GPU checks must retain their sampling limits.

**Deliverable:** CLI, isolated engine, capability probes, automatic/guided execution, and evidence-rich JSON results.

## Phase 5 — Technician interface

Build a Quickshell workflow with distinct preparation, inventory, testing/assessment, and reporting stages.

- Equipment checklist and readiness before Start.
- Quick/extended and automatic/guided controls.
- Device inventory and selected tests.
- Progress, cancellation, and clear prerequisites/errors.
- Live camera preview and microphone meter.
- Fullscreen display patterns on the selected monitor.
- Keyboard/pointer exercise areas.
- Guided preparation details, observations, and operator notes.
- Repeat checks with retained prior evidence and physical port labels.
- JSON report review and export.

Use software rendering for the interface where necessary so the UI remains usable while diagnosing graphics hardware. The frontend must not own hardware verdict logic or bypass the preflight gate.

**Deliverable:** complete technician workflow over the versioned session protocol.

## Phase 6 — Validation and release

Validate in successive layers:

1. Portable Rust regression fixtures and JSON contract checks.
2. Real Linux builds, prerequisites, permissions, device identity, and process lifecycle.
3. Quickshell runtime under the intended Qt version, X11, and Wayland.
4. Known-good and deliberately interrupted/failed physical signal paths.
5. Representative older and newer Intel Macs, then the declared model matrix.
6. Acoustic calibration, connector-by-connector coverage, and conservative outcome review.
7. Reproducible CLI/desktop packages, clean-system installation, and optional narrow privileged SMART access.

Publish supported model/distribution/capability status with evidence and explicit gaps. Supply a pinned matching Qt/Quickshell runtime where appropriate. Keep native package formats, live images, and native macOS support explicit additional deliverables if requested.

**Deliverable:** validated support matrix, equipment guide, reproducible artifacts, and release documentation.

## Completion criteria

- Equipment is explained before any functional test.
- Every discovered relevant capability has a check or an explicit unsupported/manual limitation.
- Missing kit, absent drivers, ambiguous signals, failures, and untested functions remain distinguishable.
- Automatic evidence and operator observations remain traceable in JSON.
- Cancellation and failures preserve completed evidence without leaving active workers.
- No diagnostic performs repairs or destructive configuration/storage operations.
- Declared compatibility is backed by real platform and physical-machine validation.

For the current implementation status, detailed remaining tasks, owners, dependencies, and release gates, continue with [CONTINUE.md](CONTINUE.md).
