import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: updatesPage

    property var updateData: null
    property bool checking: false
    property bool updating: false
    property bool rebootRequired: false

    function checkUpdates() {
        checking = true;
        updateData = null;
        rebootRequired = false;
        backend.checkUpdates();
        pollTimer.start();
    }

    Timer {
        id: pollTimer
        interval: 400
        repeat: true
        onTriggered: {
            backend.pollOp();
            if (!backend.opRunning) {
                pollTimer.stop();
                checking = false;
                updating = false;
                if (backend.opResult === 1) {
                    try { updateData = JSON.parse(backend.readLog()); }
                    catch(e) { updateData = {}; }
                    // Check if image upgrade staged reboot
                    if (updateData && updateData.reboot_required) {
                        rebootRequired = true;
                    }
                }
            }
        }
    }

    // Total update count
    property int totalUpdates: {
        if (!updateData) return 0;
        var n = 0;
        if (updateData.packages)  n += updateData.packages.length;
        if (updateData.flatpak)   n += updateData.flatpak.length;
        return n;
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── Top action bar ───────────────────────────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            height: 56
            color: palette.button

            RowLayout {
                anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                spacing: 12

                Label {
                    text: {
                        if (checking) return "Checking for updates…";
                        if (updating) return "Updating…";
                        if (updateData === null) return "Check for available updates";
                        if (rebootRequired)      return "Reboot required to apply system upgrade";
                        if (totalUpdates === 0)  return "Your system is up to date";
                        return totalUpdates + " update" + (totalUpdates !== 1 ? "s" : "") + " available";
                    }
                    font.pixelSize: 14
                    font.bold: true
                    Layout.fillWidth: true
                }

                BusyIndicator {
                    running: checking || updating
                    visible: checking || updating
                    implicitWidth: 24; implicitHeight: 24
                }

                Button {
                    text: "Check for Updates"
                    visible: !checking && !updating
                    onClicked: checkUpdates()
                }

                Button {
                    text: "Update All"
                    visible: !checking && !updating && totalUpdates > 0
                    highlighted: true
                    onClicked: {
                        updating = true;
                        // Run package upgrade via backend
                        backend.installApp("__upgrade_all__", "native");
                        pollTimer.start();
                    }
                }

                Button {
                    text: "🔄 Reboot Now"
                    visible: rebootRequired
                    highlighted: true
                    contentItem: Label {
                        text: "🔄 Reboot Now"
                        color: "white"
                    }
                    background: Rectangle {
                        color: "#1976d2"
                        radius: 4
                    }
                    onClicked: {
                        // systemctl reboot via backend.installApp as a hook
                    }
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        // ── Update log (shown while updating) ──────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            height: 120
            color: "#1a1a1a"
            visible: updating

            ScrollView {
                anchors.fill: parent
                contentWidth: availableWidth

                Label {
                    id: logLabel
                    width: parent.width
                    padding: 10
                    text: ""
                    color: "#e0e0e0"
                    font.family: "monospace"
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }
            }

            Timer {
                running: updating
                interval: 300
                repeat: true
                onTriggered: {
                    logLabel.text = backend.readLog();
                    // Update progress bar
                    progressBar.value = backend.opProgress / 100.0;
                }
            }
        }

        // Progress bar (during upgrade)
        ProgressBar {
            id: progressBar
            Layout.fillWidth: true
            value: 0
            visible: updating && backend.opProgress > 0
            height: 4
        }

        // ── Content area ─────────────────────────────────────────────────────
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            // Initial state: not checked yet
            Column {
                anchors.centerIn: parent
                spacing: 16
                visible: updateData === null && !checking

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "🔄"
                    font.pixelSize: 48
                }

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Press \"Check for Updates\" to get started"
                    color: palette.mid
                    font.pixelSize: 14
                }
            }

            // Up to date
            Column {
                anchors.centerIn: parent
                spacing: 16
                visible: updateData !== null && totalUpdates === 0 && !rebootRequired

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "✓"
                    font.pixelSize: 64
                    color: "#4caf50"
                }

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Your system is up to date"
                    font.pixelSize: 16
                    color: palette.mid
                }
            }

            ScrollView {
                anchors.fill: parent
                contentWidth: availableWidth
                visible: updateData !== null && totalUpdates > 0
                clip: true

                Column {
                    width: parent.width
                    topPadding: 12
                    bottomPadding: 12
                    spacing: 0

                    // ── Package updates ──────────────────────────────────────
                    Item {
                        width: parent.width
                        height: 40
                        visible: updateData && updateData.packages && updateData.packages.length > 0

                        Label {
                            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
                            text: "Packages"
                            font.pixelSize: 13
                            font.bold: true
                        }
                    }

                    Repeater {
                        model: (updateData && updateData.packages) ? updateData.packages : []

                        Rectangle {
                            width: updatesPage.width
                            height: 48
                            color: "transparent"

                            RowLayout {
                                anchors { fill: parent; leftMargin: 32; rightMargin: 16 }
                                spacing: 10

                                Rectangle {
                                    width: 8; height: 8; radius: 4
                                    color: "#ff9800"
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.name || modelData.id || JSON.stringify(modelData)
                                    elide: Text.ElideRight
                                }

                                Label {
                                    text: (modelData.current_version || "") +
                                          (modelData.new_version ? " → " + modelData.new_version : "")
                                    font.pixelSize: 11
                                    color: palette.mid
                                    visible: text.trim() !== "→"
                                }
                            }

                            Rectangle {
                                anchors { bottom: parent.bottom; left: parent.left; right: parent.right; leftMargin: 32; rightMargin: 16 }
                                height: 1; color: palette.mid; opacity: 0.12
                            }
                        }
                    }

                    // ── Flatpak updates ──────────────────────────────────────
                    Item {
                        width: parent.width
                        height: 40
                        visible: updateData && updateData.flatpak && updateData.flatpak.length > 0

                        Label {
                            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
                            text: "Flatpak"
                            font.pixelSize: 13
                            font.bold: true
                        }
                    }

                    Repeater {
                        model: (updateData && updateData.flatpak) ? updateData.flatpak : []

                        Rectangle {
                            width: updatesPage.width
                            height: 48
                            color: "transparent"

                            RowLayout {
                                anchors { fill: parent; leftMargin: 32; rightMargin: 16 }
                                spacing: 10

                                Rectangle {
                                    width: 8; height: 8; radius: 4
                                    color: "#6b5b9e"
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.name || modelData.id || JSON.stringify(modelData)
                                    elide: Text.ElideRight
                                }

                                Label {
                                    text: (modelData.current_version || "") +
                                          (modelData.new_version ? " → " + modelData.new_version : "")
                                    font.pixelSize: 11
                                    color: palette.mid
                                    visible: text.trim() !== "→"
                                }
                            }

                            Rectangle {
                                anchors { bottom: parent.bottom; left: parent.left; right: parent.right; leftMargin: 32; rightMargin: 16 }
                                height: 1; color: palette.mid; opacity: 0.12
                            }
                        }
                    }
                }
            }
        }
    }
}
