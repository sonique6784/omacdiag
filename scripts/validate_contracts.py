#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["jsonschema==4.26.0"]
# ///
"""Validate live CLI/protocol contracts without running hardware probes."""
import argparse
import json
import queue
import subprocess
import tempfile
import threading
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", default=str(ROOT / "target/debug/omacdiag"))
    args = parser.parse_args()
    validators = {}
    for name in ["report", "inventory", "plan", "request", "event"]:
        schema = json.loads((ROOT / "schemas" / f"{name}.schema.json").read_text())
        Draft202012Validator.check_schema(schema)
        validators[name] = Draft202012Validator(schema)
    assert validators["request"].is_valid({"protocol_version":1,"request_id":"retake-contract","command":"retake","test_id":"keyboard:visual","prompt_id":"pending","stage":"assess"})
    assert not validators["request"].is_valid({"protocol_version":1,"request_id":"retake-contract","command":"retake","test_id":"keyboard:visual","prompt_id":"pending","stage":"prepare"})
    validators["inventory"].validate(json.loads(subprocess.check_output([args.engine, "inventory", "--json"])))
    validators["plan"].validate(json.loads(subprocess.check_output([args.engine, "plan"])))
    fixture = Path(args.engine).parent / "examples/contract_fixture"
    if not fixture.exists():
        raise SystemExit("Build the required fixture with `cargo build --locked --examples` first")
    synthetic = json.loads(subprocess.check_output([str(fixture)]))
    validators["report"].validate(synthetic)
    assert synthetic["inventory"]["machine"]["omarchy_version"] == "4.0.4-1"
    assert synthetic["inventory"]["machine"]["board_name"] == "Mac-66F35F19FE2A0D05"
    assert synthetic["inventory"]["pci_devices"][0]["device"]["id"] == "0166"
    legacy = json.loads(json.dumps(synthetic))
    for key in ("omarchy_version", "board_name", "board_version"):
        del legacy["inventory"]["machine"][key]
    del legacy["inventory"]["pci_devices"]
    validators["report"].validate(legacy)
    assert {result["outcome"] for result in synthetic["results"]} == {"passed", "failed", "inconclusive", "blocked", "skipped", "unsupported", "cancelled"}
    assert any(result["automatic_outcome"] is not None for result in synthetic["results"])
    assert any(result["automatic_outcome"] is None for result in synthetic["results"])
    invalid = dict(synthetic, overall="healthy")
    assert not validators["report"].is_valid(invalid)
    with tempfile.TemporaryDirectory(prefix="omacdiag-contract-") as directory:
        path = Path(directory) / "session.json"
        child = subprocess.Popen([args.engine, "session", "--output", str(path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        events = queue.Queue()
        def read_events():
            for line in child.stdout:
                try:
                    events.put(json.loads(line))
                except Exception as error:
                    events.put(error)
        threading.Thread(target=read_events, daemon=True).start()
        seen = []
        pending = []
        next_request = 0
        def until(kind):
            for _ in range(100):
                for index, event in enumerate(pending):
                    if event["type"] == kind:
                        return pending.pop(index)
                event = events.get(timeout=10)
                if isinstance(event, Exception):
                    raise event
                validators["event"].validate(event)
                seen.append(event)
                if event["type"] == kind:
                    return event
                pending.append(event)
            raise AssertionError(f"No {kind} event")
        def send(command, **fields):
            nonlocal next_request
            next_request += 1
            request = dict(protocol_version=1, request_id=f"contract-{next_request}", command=command, **fields)
            validators["request"].validate(request)
            child.stdin.write(json.dumps(request) + "\n")
            child.stdin.flush()
        try:
            until("hello")
            initial_inventory = until("inventory")["payload"]
            assert "omarchy_version" in initial_inventory["machine"]
            assert "board_name" in initial_inventory["machine"]
            assert "board_version" in initial_inventory["machine"]
            assert initial_inventory["pci_devices"] is None or isinstance(initial_inventory["pci_devices"], list)
            initial_plan = until("plan")["payload"]
            # Refresh must restore the complete catalogue and preserve the chosen profile/mode.
            send("prepare", selected_tests=[], profile="extended", mode="automatic")
            prepared = until("plan")["payload"]
            assert prepared["tests"] == []
            assert prepared["profile"] == "extended" and prepared["mode"] == "automatic"
            send("refresh", readiness={"usb_media":"available"})
            refreshed = until("refreshed")["payload"]
            validators["inventory"].validate(refreshed["inventory"])
            validators["plan"].validate(refreshed["plan"])
            assert refreshed["inventory"]["machine"]["model"] == initial_inventory["machine"]["model"]
            assert refreshed["plan"]["profile"] == "extended" and refreshed["plan"]["mode"] == "automatic"
            if initial_plan["tests"]:
                assert refreshed["plan"]["tests"]
            for equipment in refreshed["plan"]["equipment"]:
                if equipment["id"] == "usb_media":
                    assert equipment["readiness"] == "available"
            saved = json.loads(path.read_text())
            assert saved["inventory"] == refreshed["inventory"]
            assert saved["plan"] == refreshed["plan"]
            before_rejection = path.read_bytes()
            send("refresh", readiness={"not_an_equipment_id":"available"})
            assert "Unknown equipment ID" in until("error")["payload"]["message"]
            assert path.read_bytes() == before_rejection
            send("prepare", selected_tests=["not_a_test"], mode="automatic")
            assert "Unknown test ID" in until("error")["payload"]["message"]
            assert path.read_bytes() == before_rejection
            child.stdin.write('{malformed json}\n')
            child.stdin.flush()
            until("error")
            child.stdin.write('{"protocol_version":1,"request_id":"unknown-field","command":"inventory","extra":true}\n')
            child.stdin.flush()
            assert "unknown field" in until("error")["payload"]["message"]
            # Explicit empty selection exercises the state machine without capture or playback.
            send("prepare", selected_tests=[], mode="automatic")
            assert until("plan")["payload"]["tests"] == []
            send("export", path=str(Path(directory) / "snapshot.json"))
            until("exported")
            # Runtime rejection is checked independently from schema validation.
            child.stdin.write('{"protocol_version":1,"request_id":"unacknowledged","command":"start"}\n')
            child.stdin.flush()
            assert "preflight" in until("error")["payload"]["message"]
            send("start", acknowledged_preflight=True)
            until("started")
            result = until("finished")["payload"]["report"]
            validators["report"].validate(result)
            assert result["overall"] == "incomplete"
            assert result["results"] == []
            validators["report"].validate(json.loads(path.read_text()))
            before_rejection = path.read_bytes()
            send("refresh")
            assert "before testing" in until("error")["payload"]["message"]
            assert path.read_bytes() == before_rejection
            send("start", acknowledged_preflight=True)
            assert "already started" in until("error")["payload"]["message"]
            send("repeat", test_id="not_a_test")
            assert "Unknown test_id" in until("error")["payload"]["message"]
            assert path.read_bytes() == before_rejection
            child.stdin.write('{"protocol_version":99,"request_id":"version","command":"inventory"}\n')
            child.stdin.flush()
            assert "version" in until("error")["payload"]["message"]
            send("quit")
            child.stdin.close()
            assert child.wait(timeout=10) == 0, child.stderr.read()
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=10)
        guided = next((test for test in initial_plan["tests"] if test["guided"] and test["probe"] == "manual" and not test["equipment"]), None)
        if guided:
            active_path = Path(directory) / "active-session.json"
            active = subprocess.Popen([args.engine, "session", "--output", str(active_path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            active_events = queue.Queue()
            def read_active():
                for line in active.stdout:
                    active_events.put(json.loads(line))
            threading.Thread(target=read_active, daemon=True).start()
            waiting = []
            def receive(kind):
                for _ in range(100):
                    for index, event in enumerate(waiting):
                        if event["type"] == kind:
                            return waiting.pop(index)
                    event = active_events.get(timeout=10)
                    validators["event"].validate(event)
                    seen.append(event)
                    if event["type"] == kind:
                        return event
                    waiting.append(event)
                raise AssertionError(f"No {kind} event in active session")
            def request(command, **fields):
                nonlocal next_request
                next_request += 1
                item = dict(protocol_version=1, request_id=f"active-{next_request}", command=command, **fields)
                validators["request"].validate(item)
                active.stdin.write(json.dumps(item) + "\n")
                active.stdin.flush()
            try:
                receive("hello"); receive("inventory"); receive("plan")
                readiness = {item:"available" for item in guided["equipment"]}
                request("prepare", selected_tests=[guided["id"]], mode="guided", readiness=readiness)
                assert [item["id"] for item in receive("plan")["payload"]["tests"]] == [guided["id"]]
                request("start", acknowledged_preflight=True)
                receive("started")
                first_started = receive("test_started")["payload"]["test"]
                assert first_started["id"] == guided["id"]
                prompt = receive("prompt")["payload"]
                assert prompt["stage"] == "assess"
                before_rejection = active_path.read_bytes()
                request("refresh")
                assert "before testing" in receive("error")["payload"]["message"]
                assert active_path.read_bytes() == before_rejection
                request("observe", test_id=guided["id"], prompt_id="stale-prompt", stage="assess", verdict="inconclusive")
                assert "does not match" in receive("error")["payload"]["message"]
                assert active_path.read_bytes() == before_rejection
                request("retake", test_id=guided["id"], prompt_id="stale-prompt", stage="assess")
                assert "Retake does not match" in receive("error")["payload"]["message"]
                assert active_path.read_bytes() == before_rejection
                request("retake", test_id=guided["id"], prompt_id=prompt["prompt_id"], stage="assess")
                receive("retaking")
                expanded = receive("plan")["payload"]
                assert len(expanded["tests"]) == 2
                assert expanded["tests"][0]["required"] is False
                second_started = receive("test_started")["payload"]["test"]
                assert second_started["id"].startswith(guided["id"] + "/retake-")
                second_prompt = receive("prompt")["payload"]
                assert second_prompt["stage"] == "assess"
                request("observe", test_id=second_started["id"], prompt_id=second_prompt["prompt_id"], stage="assess", verdict="inconclusive")
                completed = receive("finished")["payload"]["report"]
                assert completed["state"] == "completed"
                assert len(completed["results"]) == 2
                assert completed["results"][0]["reason_code"] == "operator_requested_retake"
                assert completed["results"][1]["outcome"] == "inconclusive"
                request("repeat", test_id=guided["id"])
                repeated = receive("repeat_prepared")["payload"]["plan"]
                assert len(repeated["tests"]) == 3
                assert repeated["tests"][2]["id"].startswith(guided["id"] + "/repeat-")
                checkpoint = json.loads(active_path.read_text())
                assert checkpoint["results"] == completed["results"]
                assert checkpoint["state"] == "prepared"
                before_rejection = active_path.read_bytes()
                request("refresh")
                assert "before testing" in receive("error")["payload"]["message"]
                assert active_path.read_bytes() == before_rejection
                request("quit")
                active.stdin.close()
                assert active.wait(timeout=10) == 0, active.stderr.read()
            finally:
                if active.poll() is None:
                    active.kill()
                    active.wait(timeout=10)
    print(f"Validated 5 JSON Schemas, CLI contracts, and {len(seen)} session events; no hardware tests executed.")

if __name__ == "__main__":
    main()
