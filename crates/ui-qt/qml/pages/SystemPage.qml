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
    property string opStatusMsg: ""
    property bool opSuccess: false

    // Available DE images (matching Python RAKUOS_IMAGES)
    property var deImages: [
        { label: "KDE Plasma", image_name: "rakuos-kde"    },
        { label: "GNOME",      image_name: "rakuos-gnome"  },
        { label: "COSMIC",     image_name: "rakuos-cosmic" },
    ]

    function activate() {
        if (statusData === null) loadStatus();
    }

    function loadStatus() {
        loading = true;
        statusData = null;
        opStatusMsg = "";
        backend.loadSystemStatus();
        pollTimer.start();
    }

    function startUpgrade() {
        upgrading = true;
        upgradeLog = "";
        rebootRequired = false;
        opStatusMsg = "";
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
                    opStatusMsg = "Upgrade staged. Reboot to apply.";
                    opSuccess = true;
                } else if (backend.opResult === 2) {
                    opStatusMsg = "Upgrade failed. Check log above.";
                    opSuccess = false;
                }
            }
        }
    }

    // ── Parse image name helper ───────────────────────────────────────────────
    function parseImageName(imageRef) {
        if (!imageRef) return { name: "", isNvidia: false };
        var parts = imageRef.split(":");
        var path = (parts[0] || "").replace("ghcr.io/", "").replace("docker.io/", "");
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

            // Page title
            Label {
                text: "System"
                font.pixelSize: 22
                font.bold: true
                bottomPadding: 4
            }

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

                        BusyIndicator {
                            running: upgrading
                            visible: upgrading
                            implicitWidth: 22; implicitHeight: 22
                        }

                        Button {
                            text: "Upgrade System"
                            visible: !upgrading && !rebootRequired
                            onClicked: startUpgrade()
                        }

                        Button {
                            visible: !upgrading
                            flat: true
                            implicitWidth: 90; implicitHeight: 32
                            contentItem: Label { text: "↻ Rollback"; color: "#e53935"; font.pixelSize: 12; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                            onClicked: rollbackConfirmDlg.open()
                        }

                        Button {
                            visible: rebootRequired
                            highlighted: true
                            implicitWidth: 140; implicitHeight: 32
                            background: Rectangle { color: "#1976d2"; radius: 4 }
                            contentItem: Label { text: "🔄 Reboot to Apply"; color: "white"; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                            onClicked: backend.rebootSystem()
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
                                color: root.dimText
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

                    // Status message
                    Label {
                        text: opStatusMsg
                        color: opSuccess ? "#4caf50" : "#e53935"
                        font.pixelSize: 12
                        visible: opStatusMsg !== "" && !upgrading
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
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
                        color: Qt.rgba(0, 0, 0, 0.75)
                        radius: 4
                        visible: upgrading || (upgradeLog !== "" && upgradeLog !== "\n")
                        clip: true

                        ScrollView {
                            anchors.fill: parent
                            contentWidth: availableWidth

                            Label {
                                id: upgradeLogLabel
                                width: parent.width
                                padding: 8
                                color: "#d0d0d0"
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

                        Label { text: "Current:"; color: root.dimText; font.pixelSize: 12; Layout.preferredWidth: 80 }
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

                        Label { text: "Switch to:"; color: root.dimText; font.pixelSize: 12; Layout.preferredWidth: 80 }

                        ComboBox {
                            id: deCombo
                            model: systemPage.deImages.map(function(d) { return d.label; })
                            width: 200
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
                                if (!statusData) return;
                                var selected = systemPage.deImages[deCombo.currentIndex];
                                if (!selected) return;
                                var parsed = parseImageName(statusData.image || "");
                                var isNvidia = parsed.isNvidia;
                                var newName = selected.image_name + (isNvidia ? "-nvidia" : "");
                                // Derive repo_url from current image (strip image name, replace)
                                var img = statusData.image || "";
                                var colonIdx = img.lastIndexOf(":");
                                var baseUrl = colonIdx > 0 ? img.substring(0, colonIdx) : img;
                                var slashIdx = baseUrl.lastIndexOf("/");
                                var repoBase = slashIdx > 0 ? baseUrl.substring(0, slashIdx) : baseUrl;
                                var newRepoUrl = repoBase + "/" + newName;
                                backend.upgradeImage("switch", newRepoUrl, "latest");
                                systemPage.upgrading = true;
                                upgradePollTimer.start();
                                switchStatus.text = "Switching to " + selected.label + "…";
                                switchStatus.color = root.dimText;
                            }
                        }

                        Label {
                            id: switchStatus
                            text: ""
                            color: root.dimText
                            font.pixelSize: 11
                            visible: text !== ""
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }

            // ── Installed Packages card ──────────────────────────────────────
            Rectangle {
                width: parent.width - 40
                height: pkgrootLayout.implicitHeight + 32
                radius: 8
                color: palette.button
                border.color: palette.mid
                border.width: 1
                visible: statusData !== null

                ColumnLayout {
                    id: pkgrootLayout
                    anchors { fill: parent; margins: 16 }
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            text: "Installed Packages"
                            font.pixelSize: 15
                            font.bold: true
                            Layout.fillWidth: true
                        }

                        Label {
                            text: (statusData && statusData.pkgroot_count)
                                  ? statusData.pkgroot_count + " package" + (statusData.pkgroot_count !== 1 ? "s" : "")
                                  : "0 packages"
                            color: root.dimText
                            font.pixelSize: 12
                        }

                        Item { width: 8 }

                        Button {
                            flat: true
                            implicitWidth: 110; implicitHeight: 32
                            contentItem: Label { text: "Reset Packages"; color: "#e53935"; font.pixelSize: 12; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                            onClicked: pkgrootResetConfirmDlg.open()
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                    Label {
                        text: "No packages installed."
                        color: root.dimText
                        visible: !statusData || !statusData.pkgroot_packages || statusData.pkgroot_packages.length === 0
                        font.pixelSize: 12
                    }

                    Repeater {
                        model: (statusData && statusData.pkgroot_packages) ? statusData.pkgroot_packages : []
                        Label {
                            text: "• " + modelData
                            font.pixelSize: 12
                        }
                    }

                    Label {
                        id: pkgrootStatusLbl
                        text: ""
                        color: root.dimText
                        font.pixelSize: 11
                        visible: text !== ""
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    // ── Rollback confirm dialog ───────────────────────────────────────────────
    Dialog {
        id: rollbackConfirmDlg
        title: "Rollback System?"
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel

        Label {
            text: "This will roll back to the previous OS image.\nYou will need to reboot after rollback."
            wrapMode: Text.WordWrap
            width: 320
        }

        onAccepted: {
            backend.rollbackSystem();
            upgrading = true;
            upgradeLog = "";
            opStatusMsg = "";
            upgradePollTimer.start();
        }
    }

    // ── Package root reset confirm dialog ────────────────────────────────────
    Dialog {
        id: pkgrootResetConfirmDlg
        title: "Reset Installed Packages?"
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel

        Label {
            text: "This will remove all installed packages and reset\nto the base image state.\nA reboot will be required."
            wrapMode: Text.WordWrap
            width: 320
        }

        onAccepted: {
            backend.installApp("__reset_pkgroot__", "native");
            pkgrootStatusLbl.text = "Resetting package root…";
        }
    }

}
