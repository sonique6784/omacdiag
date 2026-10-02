# Validation status and release gate

## Software checks

Run `cargo test --locked`, strict Clippy, formatting, and the contract validator. The validator checks all public schema documents, inventory/plan CLI output, refresh and its checkpoint, preflight enforcement, protocol version rejection, snapshot export, duplicate-start rejection, active-session cancellation/repeat, and empty-selection reporting without running hardware tests. Build the synthetic report example before the validator; it is a required fixture.

Regression fixtures cover PCI cameras without capture endpoints, unknown models, equipment aggregation, missing kit, unattended sessions, cancellation, private atomic reports, microphone clipping/silence, matched acoustic signals, bounded subprocess output and deadlines.

The original development host was macOS. A first real Linux build, read-only inventory check, and installed-Quickshell startup smoke test now run on a 2012 `MacBookAir5,1`; see [the Linux baseline record](linux-baseline-2026-09-30.md). This does **not** replace a guided desktop walkthrough, pinned-runtime validation, or physical hardware test. CI recipes are supplied; no remote CI results are asserted here.

## Physical Mac matrix

No model is certified yet. Record actual model identifiers, Linux distribution, kernel, firmware/driver bindings, runtime versions, test kit and JSON report for each matrix entry.

| Cohort | Coverage to establish | Status |
|---|---|---|
| 2009–2011 notebooks | USB cameras, older Intel/NVIDIA/AMD graphics, legacy jacks, optical/FireWire where present | Unverified |
| 2012–2015 notebooks | USB and PCIe cameras, Intel/hybrid graphics, card readers, display adapters | Unverified |
| 2016–2017 notebooks | USB-C/Thunderbolt, supported audio, Touch Bar where exposed | Unverified |
| 2009–2017 iMac | Panel patterns, speakers/mic/camera, SD, GPU variants, desktop power | Unverified |
| 2009–2017 Mac mini | External displays, networking, ports, desktop power | Unverified |
| 2009–2017 Mac Pro | Multiple GPUs/displays, expansion devices, networking and storage | Unverified |
| 2017 iMac Pro | Boot/runtime and driver availability must be established separately | Unverified |

The catalogue contains published connector counts for seven representative model identifiers; those entries remain physically unverified. Expand model/year/port profiles from verified sources and actual machines. Family-only profiles intentionally do not invent physical port counts or release dates. Generic unknown devices/interfaces may require operator-only or unsupported checks.

## Acceptance scenarios

- Checklist appears before any capture, playback, load or physical-device test. Deselection updates equipment requirements.
- Unavailable/uncertain equipment is skipped; automatic mode never hangs awaiting a technician.
- Unplug/reinsert known media only after the action prompt. A fixture on a system disk or different controller must be rejected.
- Test each physical connector individually; retain port labels and repeat attempts. Distinguish USB function, Thunderbolt function, display transport and charging.
- Missing FaceTime HD/Broadcom drivers or firmware do not become hardware-failure declarations. Driver setup belongs to the environment preparation process, not this application.
- Camera preview works under CPU UI rendering, remains live during guided assessment, and stops on verdict, retake, or cancellation. Compare maximum advertised and verified native capture resolution; a scaled preview is not the camera's maximum. Capture below 90% of the selected mode's advertised FPS is inconclusive. Static/dark scenes and failed capture remain distinguishable from operator-observed image defects.
- Test silent, clipped and valid microphones. In the guided response check, listen to the transient capture replay and distinguish voice from room noise; verify no audio file or mixer change remains. Test muted outputs, speaker distortion, external room noise, busy ALSA endpoints and software-loopback inputs.
- Calibrate acoustic thresholds using known good and deliberately disconnected signal paths. Require reproducible positive correlation on both emitted channel sequences. Never interpret a negative correlation as a speaker-only failure.
- Test every detected GPU by its render-node identity. Software rendering and unavailable device mapping must not pass. Inject bad readback to confirm failure handling.
- Verify EDID checks, actual fullscreen pattern placement and visual observations on internal/external monitors, including hot-unplug.
- For a keyboard backlight, verify low/high control readbacks, visible illumination change, and restoration of the original level after completion and cancellation. A readback alone is not a visual pass.
- Verify read permissions, SMART availability, unhealthy drive telemetry, failing reads and helper denial. No diagnostic writes, mounts or formatting occur.
- Confirm charging state transitions; preserve battery wear as measurements rather than an automatic component-failure verdict.
- Validate adapter-specific Bluetooth and interface-bound network peer checks with disconnected peers and blocked ICMP.
- Cancel during workers and during operator prompts; check private atomic JSON snapshots and incomplete pending assessments. A crashed worker must not stop unrelated checks.
- Complete X11 and Wayland sessions on every target distribution with the pinned runtime, including an older GPU or intentionally disabled acceleration.

## Evidence limits

Allocated-memory tests cannot certify all physical RAM. GPU pbuffer readback is a basic graphics path check, not full VRAM/3D certification. Distributed storage samples do not verify every sector. Sensor and battery telemetry do not establish accuracy or battery runtime. ICMP failure may be network policy. A sound-card endpoint may route to HDMI/headphones rather than built-in speakers; operator/physical correlation evidence must identify the actual route.

The application changes no persistent mixer/network/driver configuration. Technicians handle pairing, networking, mounting fixtures and display setup through normal desktop tools; restoration of those external actions is their workflow responsibility.
