# Architecture and delivery phases

## Responsibilities

| Subsystem | Owns | Does not own |
|---|---|---|
| `model` / `report` / schemas | Contracts, evidence, coverage, atomic persistence | Device operations or UI decisions |
| `discovery` / `profiles` | Linux enumeration, identity, driver evidence, published model expectations | Functional tests or repair |
| `planner` | Test selection, equipment aggregation, permissions, duration estimates | Executing tests |
| `engine` / `process` | Session state, prerequisite checks, workers, deadlines, cancellation | Hardware-specific evaluation |
| `probes` | Linux capability adapters, measurements, conservative evaluations | Report persistence or frontend state |
| `protocol` | Versioned process API and guided action exchange | Visual assessment |
| `ui` | Equipment checklist, previews, patterns, operator observations | Hardware access, automatic verdicts or schema ownership |
| `packaging` / `scripts` / CI | Pinned builds, contract verification, release staging | Repairing target systems or installing drivers |

## Phase 1: contracts and reporting

Implemented versioned Rust types, JSON Schemas, equipment/test-plan contracts, timestamps, coverage calculations, and atomic private report checkpoints. Empty selections and incomplete required checks cannot produce an overall pass.

## Phase 2: discovery and model expectations

Implemented sysfs/procfs discovery, PCI/USB cameras, ALSA PCM endpoints, block devices, DRM connectors/render nodes, network/Bluetooth/input devices, charging telemetry, hwmon sensors, Thunderbolt, FireWire and keyboard backlight.

Family baselines are versioned and conservative: notebooks, iMac, mini and Mac Pro. The separate published-specification catalogue adds expected components and connector counts for MacBookPro8,1, MacBookPro12,1, MacBookPro14,1, Macmini6,1/6,2, iMac12,2 and MacPro5,1, with source URLs preserved in JSON. Other models retain unknown physical layouts. Missing expected components remain inconclusive. Camera identification includes Broadcom PCIe 14e4:1570 without assuming that a capture driver exists. USB card readers can be discovered without inserted media where their product string identifies them.

Identifiers reused across releases cannot establish an exact release year. Expanding the catalogue and certifying per-port diagrams remains a physical-validation task. Reports list connector coverage gaps independently of selected-test outcome counts, and deduplicate repeated USB port labels when comparing with published counts.

## Phase 3: equipment preflight

Implemented general preparation, model/inventory-dependent equipment lists, per-test requirements, readiness states, selected-test aggregation, and mandatory acknowledgement before functional execution. Equipment availability never overrides an absent device or unavailable driver.

## Phase 4: engine and diagnostics

Tests run sequentially to prevent audio interference and device contention. Individual workers form Unix process groups. Cancelling or exceeding a deadline terminates the worker and its descendants; stdout/stderr are bounded. A test failure does not stop unrelated tests.

Implemented:

- CPU arithmetic and bounded allocated-memory patterns.
- Drive health, distributed read-only samples, optical samples and removable-media reference checks.
- Transient camera preview/content comparison, microphone PCM levels/clipping, explicit speaker playback and microphone correlation of channel-specific tones.
- Explicit per-GPU EGL/GLES2 colour-quadrant readback with software-renderer rejection.
- DRM connector/EDID checks, charging transitions and health telemetry.
- Interface-bound ICMP exchange, adapter-specific BlueZ connection verification, Thunderbolt insertion/link evidence.
- Bounded UPower keyboard-backlight brightness cycle with a separate restorer; final visual verdict remains guided.
- Operator assessment for physical/visual functions, peripherals, keyboard, trackpad and model-specific components.

Quick CPU/RAM workloads last approximately 2–3 seconds; extended CPU/RAM workloads last 120 seconds each and GPU readback lasts 60 seconds. Storage extended mode samples up to 128 MiB distributed across the device, bounded by its deadline. Camera evidence capture lasts roughly four seconds; the guided image-quality preview continues through technician assessment. The planner sums actual estimates; a full suite may exceed 5–10 minutes because operators and additional devices increase scope.

Extended mode does not implement exhaustive RAM/VRAM certification, a full disk scan, destructive writes, charging cycles, or an all-core/3D stress suite. Report evidence describes these limits. Thermal observations at or above 90°C stop load probes conservatively.

## Phase 5: Quickshell frontend

Implemented a desktop checklist, inventory, test selection, worker progress, live JPEG previews, PCM meter, fullscreen patterns, keyboard/pointer exercise area, notes, report export and retained-evidence repeat checks.

In guided mode, checks requiring technician observation are assessed after one probe run. Checks that need physical setup or probe inputs first have a distinct preparation stage; retakes repeat that setup against a fresh baseline. Automatic checks retain their probe verdict without a confirmation prompt. Results awaiting assessment are checkpointed with any provisional pass marked incomplete. A technician cannot turn a missing backend prerequisite into an automatic pass.

## Phase 6: validation and release

Core regression tests and JSON Schema/session smoke checks are available, alongside Linux distribution CI and pinned runtime/package recipes. Physical hardware testing, Quickshell runtime testing and release artifacts require the environments in [validation.md](validation.md). No physical model has been certified by building on a macOS development host.

## Diagnostic semantics

A failed check reports a failed operation or observed symptom, not a component replacement recommendation. A missing driver is blocked/unsupported/inconclusive according to the observable evidence. Battery wear and sensor readings are telemetry, not automatic hardware-failure declarations. Correlated speaker/microphone results verify the joint acoustic path; a negative result alone cannot isolate the defective component.
