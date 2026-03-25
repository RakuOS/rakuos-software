import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: systemPage

    property var statusData: null
    property bool loading: false
    property bool upgrading: false
    property bool rebootRequired: false
    property string upgradeLog: ""

    // Available DE images (matching Python RAKUOS_IMAGES list)
    property var deImages: [
        { label: "KDE Plasma", image_name: "rakuos-kde"   },
        { label: "GNOME",      image_name: "rakuos-gnome" },
    ]

    function activate() {
        if (statusData === null) loadStatus();
    }

    function loadStatus() {
        loading = true;
        statusData = null;
        backend.loadSystemStatus();
        pollTimer.start();
    }

    function startUpgrade() {
        upgrading = true;
        upgradeLog = "";
        rebootRequired = false;
        backend.upgradeSystem();
        upgradePollTimer.start();
    }

    Timer {
        id: pollTimer
        interval: 400
        repeat: true
        onTriggered: {
            backend.pollOp();
            if (!backend.opRunning) {
                pollTimer.stop();
                loading = false;
                if (backend.opResult === 1) {
                    try { statusData = JSON.parse(backend.readLog()); }
                    catch(e) { statusData = {}; }
                }
            }
        }
    }

    Timer {
        id: upgradePollTimer
        interval: 300
        repeat: true
        onTriggered: {
            backend.pollOp();
            upgradeLog = backend.readLog();
            upgradeLogLabel.text = upgradeLog;
            upgradeProgress.value = backend.opProgress / 100.0;
            if (!backend.opRunning) {
                upgradePollTimer.stop();
                upgrading = false;
                if (backend.opResult === 1) {
                    rebootRequired = true;
                }
            }
        }
    }

    // ── Parse image name helper ───────────────────────────────────────────────
    function parseImageName(imageRef) {
        if (!imageRef) return { name: "", isNvidia: false };
        var parts = imageRef.split(":");
        var path = (parts[0] || "").replace("ghcr.io/", "");
        var name = path.split("/").pop();
        return { name: name, isNvidia: name.endsWith("-nvidia") };
    }

    function currentDELabel(imageName) {
        var base = imageName.replace("-nvidia", "");
        for (var i = 0; i < deImages.length; i++) {
            if (deImages[i].image_name === base) return deImages[i].label;
        }
        return imageName;
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        Column {
            width: parent.width
            spacing: 12
            topPadding: 20
            leftPadding: 20
            rightPadding: 20
            bottomPadding: 20

            // Loading
            Item {
                width: parent.width - 40
                height: 60
                visible: loading

                Row {
                    anchors.centerIn: parent
                    spacing: 12
                    BusyIndicator { running: loading; implicitWidth: 28; implicitHeight: 28 }
                    Label { text: "Loading system info…"; anchors.verticalCenter: parent.verticalCenter }
                }
            }

            // ── Booted Image card ────────────────────────────────────────────
            Rectangle {
                id: imageCard
                width: parent.width - 40
                height: imageCardLayout.implicitHeight + 32
                radius: 8
                color: palette.button
                border.color: palette.mid
                border.width: 1
                visible: statusData !== null

                ColumnLayout {
                    id: imageCardLayout
                    anchors { fill: parent; margins: 16 }
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            text: "Booted Image"
                            font.pixelSize: 15
                            font.bold: true
                            Layout.fillWidth: true
                        }

                        Button {
                            text: "Upgrade System"
                            visible: !upgrading && !rebootRequired
                            onClicked: startUpgrade()
                        }

                        Button {
                            text: "🔄 Reboot to Apply"
                            visible: rebootRequired
                            highlighted: true
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                    // Info rows
                    Repeater {
                        model: {
                            if (!statusData) return [];
                            var img = statusData.image || "";
                            var parsed = parseImageName(img);
                            return [
                                { label: "Image",     value: img || "—" },
                                { label: "Version",   value: statusData.version   || "—" },
                                { label: "Digest",    value: statusData.digest ? (statusData.digest.substring(0, 24) + "…") : "—" },
                                { label: "Timestamp", value: statusData.timestamp || "—" },
                                { label: "Nvidia",    value: parsed.isNvidia ? "Yes 💠" : "No" },
                            ];
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            Label {
                                text: modelData.label
                                color: palette.mid
                                font.pixelSize: 12
                                Layout.preferredWidth: 80
                            }

                            Label {
                                text: modelData.value
                                font.pixelSize: 12
                                Layout.fillWidth: true
                                wrapMode: Text.NoWrap
                                elide: Text.ElideRight
                            }
                        }
                    }

                    // Upgrade progress
                    ProgressBar {
                        id: upgradeProgress
                        Layout.fillWidth: true
                        value: 0
                        visible: upgrading && value > 0
                        height: 6
                    }

                    // Upgrade log
                    Rectangle {
                        Layout.fillWidth: true
                        height: 120
                        color: "#111"
                        radius: 4
                        visible: upgrading || upgradeLog !== ""
                        clip: true

                        ScrollView {
                            anchors.fill: parent
                            contentWidth: availableWidth

                            Label {
                                id: upgradeLogLabel
                                width: parent.width
                                padding: 8
                                color: "#e0e0e0"
                                font.family: "monospace"
                                font.pixelSize: 11
                                wrapMode: Text.WordWrap
                            }
                        }
                    }
                }
            }

            // ── Desktop Environment card ─────────────────────────────────────
            Rectangle {
                width: parent.width - 40
                height: deCardLayout.implicitHeight + 32
                radius: 8
                color: palette.button
                border.color: palette.mid
                border.width: 1
                visible: statusData !== null

                ColumnLayout {
                    id: deCardLayout
                    anchors { fill: parent; margins: 16 }
                    spacing: 10

                    Label {
                        text: "Desktop Environment"
                        font.pixelSize: 15
                        font.bold: true
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Label { text: "Current:"; color: palette.mid; font.pixelSize: 12; width: 80 }
                        Label {
                            text: {
                                if (!statusData) return "—";
                                var parsed = parseImageName(statusData.image || "");
                                var lbl = currentDELabel(parsed.name);
                                return lbl + (parsed.isNvidia ? " (Nvidia)" : "");
                            }
                            font.pixelSize: 12
                            font.bold: true
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Label { text: "Switch to:"; color: palette.mid; font.pixelSize: 12; width: 80 }

                        ComboBox {
                            id: deCombo
                            model: systemPage.deImages.map(function(d) { return d.label; })
                            width: 200
                            onCurrentIndexChanged: {
                                switchBtn.enabled = true;
                                switchStatus.text = "";
                            }
                        }

                        Button {
                            id: switchBtn
                            text: "Switch DE"
                            enabled: {
                                if (!statusData) return false;
                                var parsed = parseImageName(statusData.image || "");
                                var base = parsed.name.replace("-nvidia", "");
                                var selected = systemPage.deImages[deCombo.currentIndex];
                                return selected && selected.image_name !== base;
                            }
                            onClicked: {
                                switchStatus.text = "Switch not yet wired — use bootc switch manually.";
                            }
                        }

                        Label {
                            id: switchStatus
                            text: ""
                            color: palette.mid
                            font.pixelSize: 11
                            visible: text !== ""
                        }
                    }
                }
            }

            // ── Overlay Packages card ────────────────────────────────────────
            Rectangle {
                width: parent.width - 40
                height: overlayLayout.implicitHeight + 32
                radius: 8
                color: palette.button
                border.color: palette.mid
                border.width: 1
                visible: statusData !== null

                ColumnLayout {
                    id: overlayLayout
                    anchors { fill: parent; margins: 16 }
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            text: "Overlay Packages"
                            font.pixelSize: 15
                            font.bold: true
                            Layout.fillWidth: true
                        }

                        Button {
                            text: "Reset Overlay"
                            contentItem: Label {
                                text: "Reset Overlay"
                                color: "#e53935"
                                font.pixelSize: 12
                            }
                            flat: true
                            onClicked: {
                                // Would invoke pkexec rakuos reset-overlay
                            }
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                    Label {
                        text: "No overlay packages installed."
                        color: palette.mid
                        visible: !statusData || !statusData.overlay_packages || statusData.overlay_packages.length === 0
                    }

                    Repeater {
                        model: (statusData && statusData.overlay_packages) ? statusData.overlay_packages : []

                        Label {
                            text: "• " + modelData
                            font.pixelSize: 12
                        }
                    }
                }
            }
        }
    }

    Component.onCompleted: activate()
}
