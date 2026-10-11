# omacdiag

A Linux application for inspecting Intel Macs released between 2009 and 2017: discover hardware, exercise available capabilities, and export evidence as JSON. Rust owns diagnostics and reporting; Quickshell/QML provides the technician workflow.

**Current release: development implementation.** Core regression and contract checks can run on macOS. Linux hardware, desktop rendering, packaged runtimes, and individual Mac models need physical validation before production refurbishment use. See [validation status](docs/validation.md).

## Prepare equipment before testing

Run `omacdiag equipment` for the general checklist and `omacdiag plan` for the equipment required by the detected machine and selected tests. The desktop displays this checklist before enabling Start.

| Equipment ID | Prepare |
|---|---|
| `usb_media` | Known working USB media and appropriate USB-A/USB-C connectors/adapters |
| `sd_card` | Known working compatible SD card |
| `headset` | Known working headphones and a microphone connection supported by that Mac |
| `external_display` | Powered monitor and compatible display cables/adapters |
| `charger` | Compatible known working charger or power cable |
| `network_peer` | Designated test network/peer, Wi-Fi credentials or Ethernet cable |
| `bluetooth_device` | Known working Bluetooth peripheral |
| `external_microphone` | Physical microphone if built-in capture is unavailable |
| `optical_disc` | Known working readable disc when an optical drive exists |
| `firewire_device` | Genuine FireWire peripheral and cable when applicable |
| `thunderbolt_device` | Genuine Thunderbolt peripheral and compatible cable when applicable |

Prepare reference content on USB/SD media **before** the session:

```sh
omacdiag fixture /mounted/media/omacdiag-fixture.bin
```

This explicitly creates a new 64 KiB file and refuses to overwrite an existing file. Functional diagnostics only read media; they do not mount or format it. The technician supplies the mounted fixture path. USB/SD checks verify insertion, controller ancestry, mounted-device identity, and exact reference content.

Missing or uncertain equipment creates a skipped check with a reason. Automatic mode never waits for accessories or an operator. USB success does not establish Thunderbolt function or USB-C power-delivery negotiation.

## Build and run

Rust 1.88+ is required; the lockfile is committed. Release recipes use Rust 1.93.0. Build baseline x86-64 binaries on Linux without `target-cpu=native` or AVX flags.

```sh
cargo build --locked --release
./target/release/omacdiag inventory --json
./target/release/omacdiag equipment
./target/release/omacdiag plan --profile quick --mode guided
```

Review the plan first, then explicitly acknowledge preflight:

```sh
./target/release/omacdiag run --profile quick --mode automatic \
  --acknowledge-preflight --output reports/mac-001.json

./target/release/omacdiag run --profile quick --mode guided \
  --equipment usb_media --equipment sd_card --equipment headset \
  --equipment external_display --equipment charger \
  --acknowledge-preflight --output reports/mac-002.json
```

Use repeatable `--test TEST_ID` arguments to select checks from the plan. Use `--microphone DEVICE_ID` to select the physical input for acoustic verification. Multiple microphone endpoints require explicit selection. Guided CLI preparation accepts JSON details such as `{"port_label":"left USB","fixture_path":"/media/kit/omacdiag-fixture.bin"}`, `{"peer":"192.0.2.10"}`, or `{"address":"AA:BB:CC:DD:EE:FF"}`. Assessment records pass/fail/inconclusive/skip and an operator note.

The desktop is preferred for camera preview, display patterns and keyboard/trackpad exercises. With Quickshell installed against its matching Qt version:

```sh
sh scripts/launch-ui.sh
```

The launcher uses software rendering and saves reports under `${XDG_STATE_HOME:-~/.local/state}/omacdiag/reports`. It selects a locally built engine automatically. `OMACDIAG_ENGINE` and `OMACDIAG_QUICKSHELL` can override executable locations. After a desktop session, choose **Repeat** on a result to inspect another physical connector while retaining prior evidence.

On Omarchy, the desktop reads the active `colors.toml` from `${XDG_STATE_HOME:-~/.local/state}/omarchy/current/theme` and refreshes its palette while open. It follows light and dark themes for backgrounds, text, controls, and result colors. If that file is unavailable, it uses a built-in dark palette.

Camera checks request the largest advertised V4L2 capture mode that FFmpeg can decode, at its fastest available frame rate. The report distinguishes the maximum advertised resolution from a resolution actually captured. The desktop scales only the transient preview and may use a smaller source mode for smoother live viewing during guided image-quality assessment. Capture below 90% of the selected mode's advertised frame rate is inconclusive, not a camera-failure verdict.

In Guided mode, checks requiring a technician's observation run once and then offer Pass, Fail, Inconclusive, Skip, or Retake. Automatic checks, such as CPU computation and battery telemetry, record the probe result without asking for confirmation. A check needing a physical action or probe input first shows a clearly labelled setup step; Retake repeats that setup against a new baseline. Notes are optional. Automatic mode remains unattended.

## Linux dependencies

| Capability | Dependency |
|---|---|
| Discovery, CPU/RAM, sensors, block reads | Rust executable; exposed sysfs/procfs and device permissions |
| Camera capture/preview | `ffmpeg` with V4L2 and `-fps_mode` support; working camera driver/firmware |
| Microphone/speaker tests | ALSA `arecord`, `aplay`, `amixer`; explicit usable PCM endpoints |
| Storage health | `smartctl` from smartmontools |
| GPU readback and 3D check | `libEGL.so.1`, GLES2 implementation and EGL device/render-node mapping |
| Network peer exchange | `ping` |
| Bluetooth verification | BlueZ and `busctl`; technician connects the test peripheral beforehand |
| Keyboard backlight | UPower keyboard-backlight D-Bus control through `busctl`; technician must confirm the visible change |
| Desktop | Quickshell 0.3.1 and its matching Qt runtime; Qt Quick Controls |

Missing dependencies, drivers, permissions, busy devices, disconnected displays and ambiguous evidence are reported explicitly. ALSA endpoints may be busy while a desktop audio server owns them; the application does not stop audio services or change mixer settings. Playback uses existing output volume with a bounded, louder test-only PCM signal that ends when the check ends. The guided microphone-response check replays its three-second capture once through an unambiguous analog output so the technician can hear what was recorded; neither capture nor replay audio is saved. Speaker stream delivery alone is **inconclusive**; acoustic or operator evidence is needed.

An optional narrow SMART helper can be installed by an administrator with the packaged Polkit policy. Enable it with `OMACDIAG_USE_HELPER=1`. It accepts only a whole kernel block-device basename, validates its identity, and executes an absolute `smartctl --json --all` command. Other diagnostics continue without it.

## Reports and evidence

CLI report JSON goes to stdout; progress/preflight goes to stderr. Reports are checkpointed atomically after each result, with Unix permissions `0600`. Ctrl-C, protocol cancellation and desktop termination preserve the last completed evidence. A hard kill or power loss preserves the last successful checkpoint, not the running test.

- Recognition: `detected`, `expected_not_detected`, `unknown`, `not_applicable`.
- Results: `passed`, `failed`, `inconclusive`, `blocked`, `skipped`, `unsupported`, `cancelled`.
- Overall status concerns **selected checks**, not certification of the entire computer.
- Measurements carry units. Automatic evidence and operator observations remain distinct.
- Reports preserve the automatic verdict and reason even when a technician supplies a different observation.
- UTC timestamps are Unix epoch milliseconds encoded as strings.
- Audio and image buffers remain transient and do not enter the report; JPEG previews travel only over the local process pipe.

Schemas: [report](schemas/report.schema.json), [inventory](schemas/inventory.schema.json), [plan](schemas/plan.schema.json), [requests](schemas/request.schema.json), [events](schemas/event.schema.json).

The report's `inventory.machine` includes the Omarchy package version and DMI `board_name` (Board ID) and `board_version` when available. `inventory.pci_devices` lists every device from `lspci` with its PCI address, class/vendor/device names and numeric IDs, optional subsystem IDs, revision, programming interface, and bound driver. A `null` PCI list means collection was unavailable; an empty list means no devices were reported. PCI entries provide inventory evidence and do not add diagnostic tests.

Each detected GPU offers a separate GLES2 3D check. It verifies textured geometry, shaders, depth and offscreen framebuffer pixels, then reports GL versions, limits, and synchronized 640×480 offscreen FPS. Quick mode measures for 3 seconds; extended mode for 15 seconds after warmup. FPS has no pass threshold and is not displayed refresh rate or a comparable GPU benchmark.

## Development and packaging

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --examples
uv run scripts/validate_contracts.py
```

The contract validator uses a development-only Python dependency and does not execute hardware probes. The application has no Python runtime dependency. Regenerate schemas after contract changes with `python3 scripts/generate_schemas.py`.

See [architecture and phases](docs/architecture.md), [session protocol](docs/protocol.md), [release packaging](docs/packaging.md), and [physical validation checklist](docs/validation.md).
