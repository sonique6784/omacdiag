#!/usr/bin/env python3
"""Generate public JSON Schema documents. No application runtime dependency."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STRING = {"type": "string"}
INTEGER = {"type": "integer", "minimum": 0}
TIMESTAMP = {"type": "string", "pattern": "^[0-9]+$", "description": "UTC Unix epoch milliseconds, encoded as a string"}
OUTCOMES = ["passed", "failed", "inconclusive", "blocked", "skipped", "unsupported", "cancelled"]
KINDS = "cpu memory storage usb camera microphone speaker audio_jack sd_reader gpu display battery power network bluetooth keyboard trackpad thermal fan optical firewire thunderbolt touch_bar keyboard_backlight".split()

def enum(values): return {"type": "string", "enum": values}
def array(items): return {"type": "array", "items": items}
def nullable(schema): return {"anyOf": [schema, {"type": "null"}]}
def ref(name): return {"$ref": f"#/$defs/{name}"}
def mapping(value): return {"type": "object", "additionalProperties": value}
def obj(properties, required=None):
    return {"type": "object", "additionalProperties": False, "properties": properties,
            "required": list(properties) if required is None else required}

DEFS = {}
machine_properties = dict(manufacturer=STRING, model=STRING, serial=nullable(STRING),
    omarchy_version=nullable(STRING), board_name=nullable(STRING), board_version=nullable(STRING),
    os=STRING, distribution=STRING, kernel=STRING, architecture=STRING, model_profile=nullable(STRING),
    expected_ports=mapping(INTEGER), profile_sources=array(STRING))
DEFS["machine"] = obj(machine_properties, [key for key in machine_properties if key not in
    ("omarchy_version", "board_name", "board_version")])
DEFS["pci_identifier"] = obj(dict(name=STRING, id=STRING))
DEFS["pci_device"] = obj(dict(address=STRING, **{"class": ref("pci_identifier")},
    vendor=ref("pci_identifier"), device=ref("pci_identifier"),
    subsystem_vendor=nullable(ref("pci_identifier")), subsystem_device=nullable(ref("pci_identifier")),
    revision=nullable(STRING), programming_interface=nullable(STRING), driver=nullable(STRING)))
DEFS["device"] = obj(dict(id=STRING, kind=enum(KINDS), name=STRING,
    recognition=enum(["detected", "expected_not_detected", "unknown", "not_applicable"]),
    sysfs_path=nullable(STRING), node=nullable(STRING), driver=nullable(STRING),
    attributes=mapping(STRING), evidence=array(STRING)))
inventory_properties = dict(schema_version={"const": 1}, machine=ref("machine"),
    pci_devices=nullable(array(ref("pci_device"))), devices=array(ref("device")), warnings=array(STRING))
DEFS["inventory"] = obj(inventory_properties, [key for key in inventory_properties if key != "pci_devices"])
DEFS["equipment"] = obj(dict(id=STRING, name=STRING, instructions=STRING, required_for=array(STRING),
    readiness=enum(["available", "unavailable", "uncertain"])))
DEFS["test"] = obj(dict(id=STRING, device_id=STRING, name=STRING, probe=STRING, guided={"type": "boolean"},
    required={"type": "boolean"}, equipment=array(STRING), permission=nullable(STRING), instructions=STRING,
    estimated_seconds=INTEGER, deadline_seconds=INTEGER))
DEFS["plan"] = obj(dict(profile=enum(["quick", "extended"]), mode=enum(["automatic", "guided"]),
    tests=array(ref("test")), equipment=array(ref("equipment")), estimated_seconds=INTEGER, notices=array(STRING)))
DEFS["measurement"] = obj(dict(value=True, unit=STRING))
DEFS["observation"] = obj(dict(verdict=enum(OUTCOMES), note=STRING, recorded_at=TIMESTAMP, details=True))
DEFS["result"] = obj(dict(test_id=STRING, device_id=STRING, outcome=enum(OUTCOMES), reason_code=STRING,
    automatic_outcome=nullable(enum(OUTCOMES)), automatic_reason_code=nullable(STRING),
    summary=STRING, started_at=TIMESTAMP, duration_ms=INTEGER, measurements=mapping(ref("measurement")),
    evidence=array(STRING), observation=nullable(ref("observation"))))
DEFS["coverage"] = obj(dict(selected=INTEGER, completed=INTEGER, passed=INTEGER, failed=INTEGER, incomplete=INTEGER, gaps=array(STRING)))
DEFS["report"] = obj(dict(schema_version={"const": 1}, application_version=STRING, session_id=STRING,
    started_at=TIMESTAMP, finished_at=nullable(TIMESTAMP), state=enum(["prepared", "running", "completed", "cancelled"]),
    overall=enum(["passed", "failed", "incomplete"]), inventory=ref("inventory"), plan=ref("plan"),
    results=array(ref("result")), coverage=ref("coverage")))

def document(name, body):
    return {"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": f"urn:omacdiag:{name}:v1", "$defs": DEFS, **body}

request = obj(dict(protocol_version={"const": 1}, request_id=STRING,
    command=enum(["prepare", "refresh", "start", "continue", "observe", "retake", "cancel", "inventory", "repeat", "export", "quit"]),
    profile=enum(["quick", "extended"]), mode=enum(["automatic", "guided"]),
    readiness=mapping(enum(["available", "unavailable", "uncertain"])), selected_tests=nullable(array(STRING)),
    microphone_id=nullable(STRING), acknowledged_preflight={"type": "boolean"}, test_id=nullable(STRING),
    prompt_id=nullable(STRING), stage=nullable(enum(["prepare", "assess"])), verdict=nullable(enum(OUTCOMES)),
    note=nullable(STRING), details=True, path=nullable(STRING)), ["protocol_version", "request_id", "command"])
request["allOf"] = [
    {"if": {"properties": {"command": {"const": "start"}}}, "then": {"required": ["acknowledged_preflight"], "properties": {"acknowledged_preflight": {"const": True}}}},
    {"if": {"properties": {"command": {"enum": ["continue", "observe"]}}}, "then": {"required": ["test_id", "prompt_id", "stage", "verdict"], "properties": {"test_id": STRING, "prompt_id": STRING, "verdict": enum(["passed", "failed", "inconclusive", "skipped"])}}},
    {"if": {"properties": {"command": {"const": "retake"}}}, "then": {"required": ["test_id", "prompt_id", "stage"], "properties": {"test_id": STRING, "prompt_id": STRING, "stage": {"const": "assess"}}}},
    {"if": {"properties": {"command": {"const": "repeat"}}}, "then": {"required": ["test_id"], "properties": {"test_id": STRING}}},
]
event = obj(dict(protocol_version={"const": 1}, session_id=STRING, request_id=nullable(STRING),
    type=enum(["hello", "inventory", "plan", "refreshed", "started", "test_started", "media", "prompt", "result", "finished", "error", "exported", "cancelling", "repeat_prepared", "retaking"]), payload=True))
event["allOf"] = [{"if": {"properties": {"type": {"const": "refreshed"}}}, "then": {"properties": {"payload": obj(dict(inventory=ref("inventory"), plan=ref("plan")))}}}]
event["allOf"] += [
    {"if": {"properties": {"type": {"const": name}}}, "then": {"properties": {"payload": ref(target)}}}
    for name, target in [("inventory", "inventory"), ("plan", "plan"), ("result", "result")]
]

if __name__ == "__main__":
    directory = ROOT / "schemas"
    directory.mkdir(exist_ok=True)
    for name, body in [("report", ref("report")), ("inventory", ref("inventory")), ("plan", ref("plan")),
                       ("request", request), ("event", event)]:
        (directory / f"{name}.schema.json").write_text(json.dumps(document(name, body), indent=2) + "\n")
