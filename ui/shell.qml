import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import "." as App

ShellRoot {
    id: root
    property var theme: App.Theme
    Session { id: session }
    Patterns { id: patterns }
    Connections { target: Quickshell; function onLastWindowClosed() { session.close(); } }
    property int page: 0
    function human(value) { return String(value).replace(/_/g, " "); }
    function statusColor(value) { return value === "passed" ? theme.success : value === "failed" ? theme.danger : theme.warning; }
    function measurementText(measurements) {
        if (!measurements) return "";
        return Object.keys(measurements).map(key =>
            root.human(key) + ": " + measurements[key].value + (measurements[key].unit ? " " + measurements[key].unit : "")
        ).join("\n");
    }
    function cameraMeasurement(result, key) {
        return result && result.measurements && result.measurements[key] ? result.measurements[key].value : "unverified";
    }
    function displayPatternPrompt() { return session.prompt && session.prompt.test.name.indexOf("Display patterns") >= 0; }
    function promptDetails() {
        return {port_label: portLabel.text, fixture_path: fixturePath.text, peer: peer.text,
            address: bluetoothAddress.text, tested_key_codes: inputExercise.seenKeys,
            click_events: inputExercise.clicks, scroll_events: inputExercise.scrolls,
            selected_screen: screenChoice.currentText, pattern_shown: patternsShown};
    }
    property bool patternsShown: false
    property string formTestId: ""
    Connections {
        target: session
        function onPromptChanged() {
            if (session.prompt && session.prompt.test.id !== root.formTestId) {
                root.formTestId = session.prompt.test.id;
                portLabel.text = ""; fixturePath.text = ""; peer.text = ""; bluetoothAddress.text = "";
                observationNote.text = ""; inputExercise.seenKeys = []; inputExercise.clicks = 0; inputExercise.scrolls = 0;
                root.patternsShown = false;
                screenChoice.currentIndex = -1;
            }
        }
    }
    FloatingWindow {
        id: mainWindow
        implicitWidth: 1120
        implicitHeight: 820
        minimumSize: Qt.size(800, 600)
        color: theme.background
        ColumnLayout {
            anchors.fill: parent; anchors.margins: 24; spacing: 18
            RowLayout {
                Layout.fillWidth: true
                ColumnLayout {
                    Text { text: "OMACDIAG"; color: theme.accent; font.pixelSize: 13; font.letterSpacing: 3 }
                    Text { text: session.inventory ? session.inventory.machine.model : "Intel Mac diagnostics"; color: theme.foreground; font.pixelSize: 28; font.bold: true }
                    Text { text: session.status; color: theme.secondary; font.pixelSize: 14 }
                }
                Item { Layout.fillWidth: true }
                AppButton { text: "Cancel session"; visible: session.running; onClicked: session.send("cancel") }
                AppButton { text: "Start available tests"; prominent: true; enabled: session.acknowledged && session.planReady && !session.pendingPlan && !session.running && !session.finished && session.plan && session.plan.tests.length > 0; onClicked: { root.page = 2; session.start(); } }
            }
            Rectangle {
                Layout.fillWidth: true; implicitHeight: errorLabel.implicitHeight + 20; visible: session.error !== ""; color: Qt.rgba(theme.danger.r, theme.danger.g, theme.danger.b, 0.16); radius: 8
                Text { id: errorLabel; anchors.fill: parent; anchors.margins: 10; text: session.error; color: theme.danger; wrapMode: Text.Wrap }
            }
            TabBar {
                id: tabs; Layout.fillWidth: true; currentIndex: root.page; onCurrentIndexChanged: root.page = currentIndex
                spacing: 6
                background: Rectangle { color: theme.background }
                AppTabButton { text: "1  Equipment & preparation" }
                AppTabButton { text: "2  Hardware inventory" }
                AppTabButton { text: "3  Tests & observations" }
                AppTabButton { text: "4  JSON report" }
            }
            StackLayout {
                currentIndex: root.page; Layout.fillWidth: true; Layout.fillHeight: true
                ScrollView {
                    id: equipmentScroll
                    clip: true
                    ColumnLayout {
                        width: equipmentScroll.availableWidth; spacing: 14
                        Card {
                            Layout.fillWidth: true
                            Text { text: "Prepare your test bench first"; color: theme.foreground; font.pixelSize: 21; font.bold: true }
                            Text { Layout.fillWidth: true; text: "Review the checklist before starting. Equipment is required for the tests listed underneath each item. Unavailable items leave explicit gaps in the report."; color: theme.secondary; wrapMode: Text.Wrap }
                            RowLayout {
                                Label { text: "Profile"; color: theme.secondary }
                                AppComboBox { model: ["Quick", "Extended"]; enabled: !session.running && !session.finished && session.results.length === 0; onActivated: { session.profile = currentIndex === 0 ? "quick" : "extended"; session.prepare(); } }
                                Label { text: "Workflow"; color: theme.secondary }
                                AppComboBox { model: ["Guided", "Automatic"]; enabled: !session.running && !session.finished && session.results.length === 0; onActivated: { session.mode = currentIndex === 0 ? "guided" : "automatic"; session.prepare(); } }
                            }
                            Text { text: session.plan ? session.plan.tests.length + " checks · approximately " + Math.ceil(session.plan.estimated_seconds / 60) + " minutes plus technician actions" : "Reading inventory…"; color: theme.accent }
                            AppButton {
                                text: "Refresh inventory after connecting supporting equipment"
                                enabled: !session.running && !session.finished && session.results.length === 0 && !session.pendingPlan
                                onClicked: { session.pendingPlan = true; session.planReady = false; session.acknowledged = false; session.pendingPlanRequest = session.send("refresh", {readiness: session.readiness}); }
                            }
                            Text { Layout.fillWidth: true; text: session.inventory && Object.keys(session.inventory.machine.expected_ports).length > 0 ? "Published connector checklist: " + Object.keys(session.inventory.machine.expected_ports).map(k => root.human(k) + " × " + session.inventory.machine.expected_ports[k]).join(" · ") : "Physical port layout is unknown for this model. Label and test each connector individually."; color: theme.warning; wrapMode: Text.Wrap }
                            Text { Layout.fillWidth: true; text: "Prepare media with: omacdiag fixture /mounted/media/omacdiag-fixture.bin\nProvide compatible cables and adapters for each physical connector. USB success does not verify Thunderbolt or USB-C power delivery."; color: theme.secondary; wrapMode: Text.Wrap }
                        }
                        Repeater {
                            model: session.plan ? session.plan.equipment : session.preparation
                            delegate: Card {
                                required property var modelData
                                Layout.fillWidth: true
                                RowLayout {
                                    Text { Layout.fillWidth: true; text: modelData.name; color: theme.foreground; font.pixelSize: 17; font.bold: true; wrapMode: Text.Wrap }
                                    AppComboBox {
                                        model: ["Uncertain", "Available", "Unavailable"]
                                        currentIndex: modelData.readiness === "available" ? 1 : modelData.readiness === "unavailable" ? 2 : 0
                                        enabled: !session.running && !session.finished && session.results.length === 0
                                        onActivated: session.setEquipment(modelData.id, ["uncertain", "available", "unavailable"][currentIndex])
                                    }
                                }
                                Text { Layout.fillWidth: true; text: modelData.instructions; color: theme.secondary; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; text: "Required for " + modelData.required_for.length + " selected check(s)"; color: theme.warning; wrapMode: Text.Wrap }
                            }
                        }
                        Card {
                            Layout.fillWidth: true
                            Text { text: "Physical microphone for acoustic verification"; color: theme.foreground; font.bold: true }
                            AppComboBox {
                                Layout.fillWidth: true; enabled: !session.running && !session.finished
                                property var microphones: session.inventory ? session.inventory.devices.filter(d => d.kind === "microphone" && d.recognition === "detected" && d.attributes.physical_input !== "false") : []
                                model: ["Use the sole physical input, if unambiguous"].concat(microphones.map(d => d.name))
                                onActivated: session.microphoneId = currentIndex > 0 ? microphones[currentIndex - 1].id : ""
                            }
                            Repeater { model: session.plan ? session.plan.notices : []; delegate: Text { required property var modelData; Layout.fillWidth: true; text: "• " + modelData; color: theme.secondary; wrapMode: Text.Wrap } }
                            AppCheckBox { text: "I have reviewed equipment, selected checks, and the recording/playback notices"; checked: session.acknowledged; enabled: session.planReady && !session.running && !session.finished && !session.pendingPlan; onClicked: session.acknowledged = checked }
                        }
                    }
                }
                ScrollView {
                    id: inventoryScroll
                    clip: true
                    ColumnLayout {
                        width: inventoryScroll.availableWidth; spacing: 12
                        Repeater { model: session.inventory ? session.inventory.warnings : []; delegate: Text { required property var modelData; Layout.fillWidth: true; text: modelData; color: theme.warning; wrapMode: Text.Wrap } }
                        Repeater {
                            model: session.inventory ? session.inventory.devices : []
                            delegate: Card {
                                required property var modelData
                                Layout.fillWidth: true
                                RowLayout { Text { Layout.fillWidth: true; text: modelData.name; color: theme.foreground; font.pixelSize: 17; wrapMode: Text.Wrap } Text { text: root.human(modelData.recognition); color: modelData.recognition === "detected" ? theme.accent : theme.warning } }
                                Text { Layout.fillWidth: true; text: root.human(modelData.kind) + " · " + modelData.id + "\nDriver: " + (modelData.driver || "not exposed") + "\nEndpoint: " + (modelData.node || "not exposed"); color: theme.secondary; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; visible: modelData.kind === "camera"; text: "Maximum advertised capture: " + (modelData.attributes.max_capture_width ? modelData.attributes.max_capture_width + " × " + modelData.attributes.max_capture_height + " px" : "unavailable"); color: theme.accent; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; text: modelData.evidence.join("\n"); color: theme.muted; wrapMode: Text.Wrap }
                            }
                        }
                    }
                }
                ScrollView {
                    id: testsScroll
                    clip: true
                    ColumnLayout {
                        width: testsScroll.availableWidth; spacing: 14
                        Card {
                            Layout.fillWidth: true; visible: session.running || session.prompt !== null
                            Text { Layout.fillWidth: true; text: session.currentTest; color: theme.foreground; font.pixelSize: 20; wrapMode: Text.Wrap }
                            Text { Layout.fillWidth: true; text: session.prompt ? session.prompt.test.instructions : (session.currentInstructions || "Collecting evidence…"); color: theme.secondary; wrapMode: Text.Wrap }
                            Image { Layout.fillWidth: true; Layout.preferredHeight: 240; visible: session.preview !== ""; source: session.preview; fillMode: Image.PreserveAspectFit; cache: false }
                            Text { Layout.fillWidth: true; visible: session.previewState !== ""; text: session.previewState === "live" ? "Live camera preview" + (session.previewSourceResolution ? " (" + session.previewSourceResolution + " source)" : "") + " · " + session.previewFps.toFixed(0) + " FPS" : session.previewState === "unavailable" ? "Live preview unavailable" + (session.previewError ? ": " + session.previewError : "") : "Camera preview " + session.previewState; color: session.previewState === "unavailable" ? theme.warning : theme.accent; wrapMode: Text.Wrap }
                            Text { visible: session.microphoneDb > -120; text: "Microphone: " + session.microphoneDb.toFixed(1) + " dBFS"; color: theme.accent }
                            AppProgressBar { Layout.fillWidth: true; visible: session.microphoneDb > -120; from: -90; to: 0; value: session.microphoneDb }
                            ColumnLayout {
                                Layout.fillWidth: true; visible: session.prompt !== null
                                AppField { id: portLabel; Layout.fillWidth: true; placeholderText: "Physical port label (e.g. left USB, rear upper USB)"; visible: session.prompt && session.prompt.test.probe === "usb_port" }
                                AppField { id: fixturePath; Layout.fillWidth: true; placeholderText: "Absolute mounted path to omacdiag-fixture.bin"; visible: session.prompt && ["usb_port", "sd_card"].indexOf(session.prompt.test.probe) >= 0 }
                                AppField { id: peer; Layout.fillWidth: true; placeholderText: "Designated peer IPv4/IPv6 address"; visible: session.prompt && session.prompt.test.probe === "network_peer" }
                                AppField { id: bluetoothAddress; Layout.fillWidth: true; placeholderText: "Bluetooth peripheral MAC address"; visible: session.prompt && session.prompt.test.probe === "bluetooth" }
                                RowLayout {
                                    visible: root.displayPatternPrompt()
                                    AppComboBox { id: screenChoice; model: Quickshell.screens.map(s => s.name); currentIndex: -1; onActivated: { patterns.screenIndex = currentIndex; root.patternsShown = false; } }
                                    AppButton { text: "Show fullscreen patterns"; enabled: screenChoice.currentIndex >= 0 && screenChoice.currentIndex < Quickshell.screens.length; onClicked: { patterns.screenIndex = screenChoice.currentIndex; root.patternsShown = true; patterns.show(); } }
                                }
                                Rectangle {
                                    id: inputExercise
                                    Layout.fillWidth: true; Layout.preferredHeight: 100; color: theme.field; radius: 8
                                    visible: session.prompt && ["keyboard", "trackpad"].some(k => session.prompt.test.device_id.indexOf(k + ":") === 0)
                                    property var seenKeys: []
                                    property int clicks: 0
                                    property int scrolls: 0
                                    focus: true
                                    Keys.onPressed: event => { var copy = seenKeys.slice(); if (copy.indexOf(event.key) < 0) copy.push(event.key); seenKeys = copy; event.accepted = true; }
                                    MouseArea {
                                        anchors.fill: parent
                                        acceptedButtons: Qt.AllButtons
                                        onClicked: { parent.clicks++; parent.forceActiveFocus(); }
                                        onWheel: { parent.scrolls++; }
                                    }
                                    Text { anchors.centerIn: parent; text: "Click here and exercise keys, pointer and scrolling\nUnique keys: " + parent.seenKeys.length + " · clicks: " + parent.clicks + " · scroll events: " + parent.scrolls; color: theme.secondary; horizontalAlignment: Text.AlignHCenter }
                                }
                                AppTextArea { id: observationNote; Layout.fillWidth: true; placeholderText: "Optional: what did you observe?"; wrapMode: TextEdit.Wrap; implicitHeight: 80 }
                                Text { Layout.fillWidth: true; visible: session.prompt && session.prompt.result; text: session.prompt && session.prompt.result ? "Automatic evidence: " + session.prompt.result.summary : ""; color: theme.warning; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; visible: session.prompt && session.prompt.test.probe === "camera_quality"; text: session.prompt ? "Maximum advertised: " + root.cameraMeasurement(session.prompt.result, "maximum_advertised_resolution") + " · Captured: " + root.cameraMeasurement(session.prompt.result, "verified_capture_resolution") : ""; color: theme.secondary; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; visible: session.prompt && session.prompt.result && root.measurementText(session.prompt.result.measurements) !== ""; text: session.prompt && session.prompt.result ? root.measurementText(session.prompt.result.measurements) : ""; color: theme.secondary; wrapMode: Text.Wrap }
                                RowLayout {
                                    AppButton { text: "Start check"; prominent: true; visible: session.prompt && session.prompt.stage === "prepare"; onClicked: session.reply("inconclusive", observationNote.text, root.promptDetails()) }
                                    AppButton { text: "Pass"; prominent: true; visible: session.prompt && session.prompt.stage === "assess"; enabled: session.pendingRetakeRequest === "" && (!root.displayPatternPrompt() || (root.patternsShown && screenChoice.currentIndex >= 0)); onClicked: session.reply("passed", observationNote.text, root.promptDetails()) }
                                    AppButton { text: "Fail"; visible: session.prompt && session.prompt.stage === "assess"; enabled: session.pendingRetakeRequest === ""; onClicked: session.reply("failed", observationNote.text, root.promptDetails()) }
                                    AppButton { text: "Inconclusive"; visible: session.prompt && session.prompt.stage === "assess"; enabled: session.pendingRetakeRequest === ""; onClicked: session.reply("inconclusive", observationNote.text, root.promptDetails()) }
                                    AppButton { text: "Skip"; enabled: session.pendingRetakeRequest === ""; onClicked: session.reply("skipped", observationNote.text, root.promptDetails()) }
                                    AppButton { text: "Retake"; visible: session.prompt && session.prompt.stage === "assess"; enabled: session.pendingRetakeRequest === ""; onClicked: session.retake() }
                                }
                            }
                        }
                        Repeater {
                            model: session.results
                            delegate: Card {
                                required property var modelData
                                Layout.fillWidth: true
                                RowLayout { Text { Layout.fillWidth: true; text: modelData.test_id; color: theme.foreground; wrapMode: Text.Wrap } Text { text: root.human(modelData.outcome); color: root.statusColor(modelData.outcome); font.bold: true } }
                                Text { Layout.fillWidth: true; text: modelData.summary; color: theme.secondary; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; visible: root.measurementText(modelData.measurements) !== ""; text: root.measurementText(modelData.measurements); color: theme.secondary; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; text: modelData.reason_code; color: theme.muted; wrapMode: Text.Wrap }
                                Text { Layout.fillWidth: true; visible: !!modelData.automatic_outcome; text: "Automatic evidence: " + (modelData.automatic_outcome || "") + " · " + (modelData.automatic_reason_code || ""); color: theme.muted; wrapMode: Text.Wrap }
                                AppButton { text: "Repeat for another connector / observation"; visible: session.finished; onClicked: { session.send("repeat", {test_id: modelData.test_id}); root.page = 0; } }
                            }
                        }
                        Text { text: "Select checks before starting"; color: theme.foreground; font.pixelSize: 18; visible: !session.running && !session.finished }
                        Repeater {
                            model: session.catalogue
                            delegate: AppCheckBox { required property var modelData; Layout.fillWidth: true; text: modelData.name + (modelData.guided ? (session.mode === "guided" ? " · technician verdict" : " · skipped in automatic mode") : ""); visible: !session.running && !session.finished; enabled: session.results.length === 0; checked: !session.disabledTests[modelData.id]; onClicked: session.setSelected(modelData.id, checked) }
                        }
                    }
                }
                ScrollView {
                    id: reportScroll
                    clip: true
                    ColumnLayout {
                        width: reportScroll.availableWidth; spacing: 14
                        Card {
                            Layout.fillWidth: true
                            Text { text: session.finalReport ? "Selected checks: " + root.human(session.finalReport.overall) : "Reports are saved after every completed check"; color: theme.foreground; font.pixelSize: 21 }
                            Text { Layout.fillWidth: true; text: "Current report: " + session.reportPath; color: theme.accent; wrapMode: Text.Wrap }
                            Text { Layout.fillWidth: true; text: "A pass applies to the selected checks. Unknown model layouts, unsampled memory/storage and untested physical connectors remain outside that evidence."; color: theme.secondary; wrapMode: Text.Wrap }
                            AppField { id: exportPath; Layout.fillWidth: true; placeholderText: "Export path, e.g. /home/technician/reports/mac-001.json" }
                            AppButton { text: "Export JSON snapshot"; enabled: exportPath.text !== ""; onClicked: session.send("export", {path: exportPath.text}) }
                        }
                        AppTextArea { Layout.fillWidth: true; readOnly: true; text: session.finalReport ? JSON.stringify(session.finalReport, null, 2) : "The final report will appear here. Partial results are already saved."; color: theme.secondary; font.family: "monospace"; wrapMode: TextEdit.Wrap }
                    }
                }
            }
        }
    }
}
