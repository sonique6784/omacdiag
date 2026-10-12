import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: session
    signal stopped()
    property var inventory: null
    property var plan: null
    property var preparation: []
    property var catalogue: []
    property var readiness: ({})
    property var disabledTests: ({})
    property var results: []
    property var prompt: null
    property var finalReport: null
    property string reportPath: ""
    property string profile: "quick"
    property string mode: "guided"
    property string microphoneId: ""
    property string status: "Preparing inventory"
    property string error: ""
    property string currentTest: ""
    property string currentTestId: ""
    property string currentInstructions: ""
    property string preview: ""
    property string previewState: ""
    property string previewError: ""
    property string previewSourceResolution: ""
    property real previewFps: 0
    property real microphoneDb: -120
    property bool running: false
    property bool finished: false
    property bool closing: false
    property bool pendingPlan: false
    property bool planReady: false
    property string pendingPlanRequest: ""
    property string pendingStartRequest: ""
    property string pendingRetakeRequest: ""
    property bool acknowledged: false
    property int requestNumber: 0

    function send(command, fields) {
        var request = fields || {};
        request.protocol_version = 1;
        request.request_id = "ui-" + (++requestNumber);
        request.command = command;
        backend.write(JSON.stringify(request) + "\n");
        return request.request_id;
    }
    function prepare() {
        if (running || finished || !plan || results.length > 0) return;
        pendingPlan = true;
        planReady = false;
        acknowledged = false;
        pendingPlanRequest = send("prepare", {profile: profile, mode: mode, readiness: readiness,
            selected_tests: catalogue.filter(t => !disabledTests[t.id]).map(t => t.id)});
    }
    function setEquipment(id, value) {
        var next = JSON.parse(JSON.stringify(readiness)); next[id] = value; readiness = next; prepare();
    }
    function setSelected(id, enabled) {
        var next = JSON.parse(JSON.stringify(disabledTests)); next[id] = !enabled; disabledTests = next; prepare();
    }
    function start() {
        if (!acknowledged || pendingPlan || !planReady || running || finished || !plan || plan.tests.length === 0) return;
        running = true; error = ""; status = "Preparing selected checks";
        pendingStartRequest = send("start", {acknowledged_preflight: true, microphone_id: microphoneId || null});
    }
    function reply(verdict, note, details) {
        if (!prompt || pendingRetakeRequest !== "") return;
        send(prompt.stage === "prepare" ? "continue" : "observe", {
            test_id: prompt.test.id, prompt_id: prompt.prompt_id, stage: prompt.stage,
            verdict: verdict, note: note, details: details || {}});
        prompt = null;
        if (previewState !== "") previewState = "stopping";
    }
    function retake() {
        if (!prompt || prompt.stage !== "assess" || pendingRetakeRequest !== "") return;
        pendingRetakeRequest = send("retake", {
            test_id: prompt.test.id, prompt_id: prompt.prompt_id, stage: "assess"});
    }
    function close() {
        if (closing) return;
        closing = true;
        if (backend.running) {
            send("quit");
            stopTimeout.start();
        }
        else stopped();
    }
    function receive(event) {
        if (event.protocol_version !== 1) { error = "Unsupported engine protocol"; return; }
        var p = event.payload;
        switch (event.type) {
        case "hello": preparation = p.preparation; reportPath = p.report_path; break;
        case "inventory": inventory = p; status = "Review equipment before testing"; break;
        case "refreshed":
            if (event.request_id !== pendingPlanRequest) break;
            inventory = p.inventory; plan = p.plan; catalogue = p.plan.tests; disabledTests = ({});
            pendingPlan = false; planReady = true; pendingPlanRequest = ""; acknowledged = false;
            status = "Inventory refreshed · review equipment and selected checks"; break;
        case "plan":
            if (event.request_id && event.request_id !== pendingPlanRequest) break;
            plan = p; if (catalogue.length === 0) catalogue = p.tests;
            pendingPlan = false; planReady = true; pendingPlanRequest = ""; break;
        case "repeat_prepared": plan = p.plan; planReady = true; finished = false; acknowledged = false; status = "Repeat queued · review preparation and start"; break;
        case "started": pendingStartRequest = ""; if (!finished) running = true; break;
        case "test_started": currentTest = p.test.name; currentTestId = p.test.id; currentInstructions = p.test.instructions; preview = ""; previewState = ""; previewError = ""; previewSourceResolution = ""; previewFps = 0; microphoneDb = -120; break;
        case "retaking":
            if (event.request_id !== pendingRetakeRequest) break;
            pendingRetakeRequest = ""; prompt = null; preview = ""; previewState = ""; previewFps = 0; status = "Retaking check"; break;
        case "prompt": prompt = p; currentTest = p.test.name; currentTestId = p.test.id; status = p.stage === "prepare" ? "Set up this check before its single run" : "Assess the completed check"; break;
        case "media":
            if (p.test_id !== currentTestId) break;
            if (p.data.preview) preview = p.data.preview;
            if (p.data.preview_state !== undefined) {
                previewState = p.data.preview_state;
                if (previewState === "starting" || previewState === "unavailable" || previewState === "stopped") preview = "";
            }
            if (p.data.preview_error) previewError = p.data.preview_error;
            if (p.data.preview_source_resolution) previewSourceResolution = p.data.preview_source_resolution;
            if (p.data.preview_fps !== undefined) previewFps = p.data.preview_fps;
            if (p.data.rms_dbfs !== undefined) microphoneDb = p.data.rms_dbfs;
            break;
        case "result":
            var next = results.filter(r => r.test_id !== p.test_id); next.push(p); results = next;
            break;
        case "finished": finalReport = p.report; reportPath = p.path; running = false; finished = true; prompt = null; pendingRetakeRequest = ""; preview = ""; previewState = ""; status = p.report.state === "cancelled" ? "Cancelled · partial report saved" : "Session complete · report saved"; break;
        case "exported": reportPath = p.path; status = "Report exported"; break;
        case "error":
            error = p.message;
            if (event.request_id && event.request_id === pendingPlanRequest) {
                pendingPlan = false; planReady = false; pendingPlanRequest = ""; acknowledged = false;
            }
            if (event.request_id && event.request_id === pendingStartRequest) {
                running = false; pendingStartRequest = "";
            }
            if (event.request_id && event.request_id === pendingRetakeRequest) pendingRetakeRequest = "";
            if (p.fatal) { running = false; finished = true; }
            break;
        }
    }
    property Process backend: Process {
        command: [Quickshell.env("OMACDIAG_ENGINE") || "/usr/bin/omacdiag", "session"]
        running: true
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => {
                try { session.receive(JSON.parse(line)); }
                catch (e) { session.error = "Engine message could not be decoded: " + e; }
            }
        }
        stderr: SplitParser { onRead: line => console.warn("omacdiag: " + line) }
        onExited: (code, exitStatus) => {
            stopTimeout.stop();
            session.running = false;
            if (session.closing) { session.stopped(); return; }
            if (!session.finished) session.error = "Engine stopped (" + code + "). The last saved report is at " + session.reportPath;
        }
    }
    property Timer stopTimeout: Timer {
        interval: 3000
        onTriggered: if (backend.running) backend.running = false
    }
}
