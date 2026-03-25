import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "../components"

Item {
    id: updatesPage

    property var updateData: null
    property bool checking: false
    property bool updating: false
    property bool rebootRequired: false

    function activate() {
        if (updateData === null && !checking) {
            checkUpdates();
        }
    }

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
        if (updateData.appimages) n += updateData.appimages.length;
        if (updateData.image_available) n += 1;
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
                    text: "↻  Check for Updates"
                    visible: !checking && !updating
                    onClicked: checkUpdates()
                }

                Button {
                    text: "⬆  Update All"
                    visible: !checking && !updating && totalUpdates > 0 && !rebootRequired
                    highlighted: true
                    onClicked: _doUpdateAll()
                }

                Button {
                    text: "🔄 Reboot Now"
                    visible: rebootRequired
                    highlighted: true
                    background: Rectangle { color: "#1976d2"; radius: 4 }
                    contentItem: Label { text: "🔄 Reboot Now"; color: "white"; font.pixelSize: 13 }
                    onClicked: backend.rebootSystem()
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        // ── Update log (shown while updating) ──────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            height: 120
            color: Qt.rgba(0, 0, 0, 0.75)
            visible: updating

            ScrollView {
                anchors.fill: parent
                contentWidth: availableWidth

                Label {
                    id: logLabel
                    width: parent.width
                    padding: 10
                    text: ""
                    color: "#d0d0d0"
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
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Your system and all apps are up to date."
                    font.pixelSize: 12
                    color: palette.mid
                }
            }

            ScrollView {
                anchors.fill: parent
                contentWidth: availableWidth
                visible: updateData !== null && (totalUpdates > 0 || rebootRequired)
                clip: true

                Column {
                    width: parent.width
                    topPadding: 12
                    bottomPadding: 24
                    leftPadding: 24
                    rightPadding: 24
                    spacing: 16

                    // ── OS Image update card ───────────────────────────────────
                    Rectangle {
                        width: parent.width - 48
                        height: imageCardCol.implicitHeight + 24
                        radius: 8
                        color: palette.button
                        border.color: palette.mid
                        border.width: 1
                        visible: updateData && updateData.image_available === true

                        Column {
                            id: imageCardCol
                            anchors { fill: parent; margins: 12 }
                            spacing: 8

                            RowLayout {
                                width: parent.width

                                Label {
                                    text: "Operating System"
                                    font.pixelSize: 14
                                    font.bold: true
                                    Layout.fillWidth: true
                                }
                                Label {
                                    text: "1 update"
                                    color: palette.mid
                                    font.pixelSize: 12
                                }
                            }

                            Rectangle { width: parent.width; height: 1; color: palette.mid; opacity: 0.3 }

                            RowLayout {
                                width: parent.width
                                spacing: 10

                                Label { text: "🖥"; font.pixelSize: 20 }

                                Column {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    Label {
                                        text: "RakuOS"
                                        font.pixelSize: 13
                                        font.bold: true
                                    }
                                    Label {
                                        text: {
                                            if (!updateData || !updateData.image_info) return "";
                                            var info = updateData.image_info;
                                            var t = info.update_type || info.new_tag ? "switch" : "upgrade";
                                            if (t === "switch" && info.new_tag) {
                                                return (info.current_version || "current") + "  →  " + (info.new_version || info.new_tag);
                                            }
                                            return "Refresh of " + (info.current_version || "current");
                                        }
                                        color: palette.mid
                                        font.pixelSize: 11
                                    }
                                }

                                Button {
                                    id: imageUpdateBtn
                                    text: (updateData && updateData.image_info &&
                                           updateData.image_info.new_tag) ? "Update" : "Apply Hotfix"
                                    visible: !rebootRequired
                                    onClicked: {
                                        if (!updateData || !updateData.image_info) return;
                                        var info = updateData.image_info;
                                        var utype = (info.new_tag && info.new_tag !== "") ? "switch" : "upgrade";
                                        backend.upgradeImage(utype, info.repo_url || "", info.new_tag || "");
                                        updating = true;
                                        pollTimer.start();
                                    }
                                }
                            }
                        }
                    }

                    // ── Applications section (GUI packages + flatpak + appimages) ─
                    UpdateSection {
                        visible: appPackages.length > 0
                        title: "Applications"
                        packages: {
                            if (!updateData) return [];
                            var result = [];
                            if (updateData.packages) {
                                updateData.packages.forEach(function(p) {
                                    if (p.gui) result.push(Object.assign({}, p, {pkg_type: "rpm"}));
                                });
                            }
                            if (updateData.flatpak) {
                                updateData.flatpak.forEach(function(p) {
                                    if (!p.runtime) result.push(Object.assign({}, p, {pkg_type: "flatpak"}));
                                });
                            }
                            if (updateData.appimages) {
                                updateData.appimages.forEach(function(p) {
                                    result.push(Object.assign({}, p, {pkg_type: "appimage"}));
                                });
                            }
                            return result;
                        }
                        property var appPackages: packages
                        onUpdateAllClicked: _doSectionUpdate(packages)
                    }

                    // ── Flatpak runtimes / add-ons ────────────────────────────
                    UpdateSection {
                        visible: runtimePkgs.length > 0
                        title: "Add-ons"
                        packages: {
                            if (!updateData || !updateData.flatpak) return [];
                            return updateData.flatpak.filter(function(p) { return p.runtime; })
                                .map(function(p) { return Object.assign({}, p, {pkg_type: "flatpak"}); });
                        }
                        property var runtimePkgs: packages
                        onUpdateAllClicked: _doSectionUpdate(packages)
                    }

                    // ── System packages ───────────────────────────────────────
                    UpdateSection {
                        visible: sysPkgs.length > 0
                        title: "System Dependencies"
                        packages: {
                            if (!updateData || !updateData.packages) return [];
                            return updateData.packages.filter(function(p) { return !p.gui; })
                                .map(function(p) { return Object.assign({}, p, {pkg_type: "rpm"}); });
                        }
                        property var sysPkgs: packages
                        onUpdateAllClicked: _doSectionUpdate(packages)
                    }
                }
            }
        }
    }

    // ── Update helpers ────────────────────────────────────────────────────────

    function _doUpdateAll() {
        if (updating || checking) return;
        updating = true;
        // Run flatpak update + package upgrade
        backend.upgradePackages();
        pollTimer.start();
    }

    function _doSectionUpdate(pkgs) {
        if (updating || checking) return;
        if (!pkgs || pkgs.length === 0) return;
        updating = true;
        var hasFlatpak = pkgs.some(function(p) { return p.pkg_type === "flatpak"; });
        var hasRpm     = pkgs.some(function(p) { return p.pkg_type === "rpm"; });
        if (hasFlatpak) {
            // Update all flatpaks
            backend.installApp("__upgrade_all__", "flatpak");
        } else if (hasRpm) {
            backend.upgradePackages();
        }
        pollTimer.start();
    }

    // ── UpdateSection component ───────────────────────────────────────────────

    component UpdateSection: Rectangle {
        id: secRoot
        property string title: ""
        property var packages: []
        signal updateAllClicked(var pkgs)

        width: parent ? parent.width - 48 : 400
        height: secCol.implicitHeight + 24
        radius: 8
        color: palette.button
        border.color: palette.mid
        border.width: 1

        Column {
            id: secCol
            anchors { fill: parent; margins: 12 }
            spacing: 0

            RowLayout {
                width: parent.width

                Label {
                    text: secRoot.title
                    font.pixelSize: 14
                    font.bold: true
                    Layout.fillWidth: true
                }

                Label {
                    text: secRoot.packages.length + " update" + (secRoot.packages.length !== 1 ? "s" : "")
                    color: palette.mid
                    font.pixelSize: 12
                }

                Item { width: 12 }

                Button {
                    text: "Update All"
                    onClicked: secRoot.updateAllClicked(secRoot.packages)
                }
            }

            Item { width: parent.width; height: 8 }
            Rectangle { width: parent.width; height: 1; color: palette.mid; opacity: 0.2 }

            Repeater {
                model: secRoot.packages

                Column {
                    width: secCol.width
                    spacing: 0

                    Rectangle {
                        width: parent.width
                        height: pkgRowLayout.implicitHeight + 16
                        color: "transparent"

                        RowLayout {
                            id: pkgRowLayout
                            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: 4; rightMargin: 4 }
                            spacing: 10

                            AppIcon {
                                iconPath: modelData.icon_path || ""
                                iconName: modelData.name || modelData.id || "?"
                                size: 32
                            }

                            Column {
                                Layout.fillWidth: true
                                spacing: 2

                                Label {
                                    text: modelData.name || modelData.id || ""
                                    font.pixelSize: 13
                                    font.bold: true
                                    elide: Text.ElideRight
                                    width: parent.width
                                }

                                RowLayout {
                                    spacing: 6
                                    Label {
                                        text: {
                                            var cur = modelData.current_version || modelData.version || "";
                                            var nw = modelData.new_version || modelData.version || "";
                                            if (cur && nw && cur !== nw) return cur + "  →  " + nw;
                                            if (nw) return "→  " + nw;
                                            return "";
                                        }
                                        font.pixelSize: 11
                                        color: palette.mid
                                        visible: text !== ""
                                    }
                                    Rectangle {
                                        visible: modelData.pkg_type === "flatpak"
                                        radius: 3
                                        color: "#1a237e"
                                        width: flatpakLbl.implicitWidth + 8
                                        height: 16
                                        Label {
                                            id: flatpakLbl
                                            anchors.centerIn: parent
                                            text: "Flatpak"
                                            font.pixelSize: 9
                                            color: "white"
                                        }
                                    }
                                    Rectangle {
                                        visible: modelData.pkg_type === "appimage"
                                        radius: 3
                                        color: "#e65100"
                                        width: aiLbl.implicitWidth + 8
                                        height: 16
                                        Label {
                                            id: aiLbl
                                            anchors.centerIn: parent
                                            text: "AppImage"
                                            font.pixelSize: 9
                                            color: "white"
                                        }
                                    }
                                }
                            }

                            Button {
                                text: "Update"
                                flat: true
                                onClicked: {
                                    var pkg = modelData;
                                    if (pkg.pkg_type === "flatpak") {
                                        backend.installApp(pkg.id || pkg.app_id || "", "flatpak");
                                    } else {
                                        backend.upgradePackages();
                                    }
                                    updatesPage.updating = true;
                                    pollTimer.start();
                                }
                            }
                        }
                    }

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: palette.mid
                        opacity: 0.12
                        visible: index < secRoot.packages.length - 1
                    }
                }
            }
        }
    }

}
