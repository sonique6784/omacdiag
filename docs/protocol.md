# Session protocol v1

Start `omacdiag session --output /writable/path/report.json`. Send one JSON object per stdin line. Controller events use stdout exclusively; logs use stderr. Maximum request line: 64 KiB. Worker preview/event lines are limited to 256 KiB.

Every request carries `protocol_version: 1`, a string `request_id`, and `command`. Responses include protocol version, session ID, nullable request ID, type and payload. Asynchronous events have a null request ID. Unknown versions, unknown fields and mismatched pending prompts are rejected.

Startup emits `hello`, `inventory`, and `plan`. `hello` includes the general equipment checklist and report path. Discovery and preparing a plan never start functional tests.

```json
{"protocol_version":1,"request_id":"prepare-1","command":"prepare","profile":"quick","mode":"guided","readiness":{"usb_media":"available","sd_card":"unavailable"}}
```

Readiness values are `available`, `unavailable`, or `uncertain`. An omitted `selected_tests` means all planned checks; an empty array selects none. Preparing is permitted only before evidence exists. Start another process for a different session/profile after results exist.

```json
{"protocol_version":1,"request_id":"start-1","command":"start","acknowledged_preflight":true,"microphone_id":"microphone:pcmC0D0c"}
```

Start is rejected without explicit preflight acknowledgement or after a run already began. Automatic mode emits skipped results for guided tests without waiting.

In guided mode, only checks requiring technician observation produce an `assess` prompt after one run. Automatic checks record their probe verdict without a prompt. Some guided checks first need a `prepare` prompt to collect physical setup or probe inputs (for example, insert media after a fresh baseline, enter a peer address, or get ready to listen during a short playback). This is setup, not an earlier probe run. `prompt` events include `test`, `stage`, a unique `prompt_id`, and an optional automatic `result`. For a `prepare` stage, perform the setup and reply:

```json
{"protocol_version":1,"request_id":"action-1","command":"continue","test_id":"usb:usb1/port","prompt_id":"COPY_FROM_EVENT","stage":"prepare","verdict":"inconclusive","details":{"port_label":"left USB","fixture_path":"/media/kit/omacdiag-fixture.bin"}}
```

Preparation details are preserved in the final observation. Networking uses a numeric `peer` address; Bluetooth uses the peripheral's `address`. For `assess`, record what happened after execution:

```json
{"protocol_version":1,"request_id":"assessment-1","command":"observe","test_id":"usb:usb1/port","prompt_id":"COPY_FROM_EVENT","stage":"assess","verdict":"passed","note":"Known media readable in left USB"}
```

Operator verdicts: `passed`, `failed`, `inconclusive`, `skipped`. The observation note is optional, and the desktop does not require an exercise checkbox. Automatic failed/blocked/unsupported evidence cannot be upgraded to a pass by an operator. Manual-only tests record operator evidence explicitly.

To run any check again before answering, send `retake` with the pending assessment's `test_id`, `prompt_id`, and `stage: "assess"`. A successful request emits `retaking`, then repeats any required setup with a fresh baseline before another `test_started` and `assess` prompt. Each attempt is retained with a distinct `/retake-N` test ID; only the latest attempt is required for completion, and prior automatic failures remain visible in the report. Automatic mode remains unattended and does not request a technician verdict.

Results retain `automatic_outcome` and `automatic_reason_code` independently of the combined outcome and operator observation. These fields are null for manual-only checks and checks skipped before a worker runs. An operator skip cannot erase an automatic failure.

After connecting supporting equipment, send `refresh` before starting any tests. It rediscovers hardware, preserves the profile and mode, and rebuilds the complete test selection. Supply the current `readiness` map. The `refreshed` response contains `inventory` and `plan`; review the selection and acknowledge preflight again. Insertion tests still require their fixture to be disconnected before collecting the baseline, as directed by each prompt.

Other commands:

| Command | Fields / behaviour |
|---|---|
| `inventory` | Return initial inventory |
| `refresh` | Before testing only; optional `readiness`; rediscover equipment and reset the test selection |
| `cancel` | Stop active workers and save partial evidence |
| `export` | Optional `path`; atomically write the current snapshot |
| `repeat` | `test_id`; after completion/cancellation append a new independently identified attempt and retain earlier results |
| `quit` | Cancel, finish checkpointing and exit |

Repeat emits `repeat_prepared`; review and acknowledge preflight again, then start. Only newly pending tests execute. Physical port labels distinguish attempts. The session does not infer that all ports were exercised merely because one controller passed.

`media` events carry `test_id` and transient `data` such as `preview` (JPEG data URI) or `rms_dbfs`. Camera quality previews additionally use optional `preview_state` (`starting`, `live`, `unavailable`, `stopped`), `preview_fps`, `preview_source_resolution`, and `preview_error`. The UI should ignore media for a different active test. Image buffers never enter persisted reports. Camera results include maximum advertised and verified capture resolutions as separate measurements; an unverified resolution is omitted rather than guessed. Captured FPS below 90% of the requested mode's advertised FPS produces an inconclusive automatic result, which a guided observation cannot upgrade to a pass. `finished` includes the report snapshot and saved path. `error` explains rejected commands or persistence/process failures.
