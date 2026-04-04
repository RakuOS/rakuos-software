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
    property bool imageUpdateDone: false

    // Update queue: ["packages", "flatpak", "image"] in order
    property var updateQueue: []
    property int queueStep: 0
    property bool inQueueMode: false
    property bool pendingOpIsCheck: false
    property string currentOpLabel: ""
    // Which update type is actively running: "packages", "flatpak", "image", or a specific app_id
    property string activeUpdateType: ""

    // Tracks successfully completed updates so rows can disappear.
    // Keys: specific app_id, "__packages__" for all rpm rows, "__flatpak__" for all flatpak rows.
    property var completedUpdates: ({})

    // On activate: read daemon cache — don't trigger our own check.
    // If the cache doesn't exist yet the daemon is still doing its first
    // check, so show "Checking…" and poll until it appears.
    function activate() {
        _loadFromCache();
    }

    function _loadFromCache() {
        var cached = backend.loadUpdatesCache();
        if (cached && cached.length > 0) {
            daemonPollTimer.stop();
            checking = false;
            try {
                updateData = JSON.parse(cached);
                rebootRequired = updateData.reboot_required === true;
                // Keep badge in sync
                backend.pendingUpdateCount = totalUpdates;
            } catch(e) { updateData = {}; }
        } else {
            // Daemon hasn't written cache yet — wait for it
            checking = true;
            updateData = null;
            daemonPollTimer.start();
        }
    }

    // Manual refresh — runs a full check via the backend op system
    function checkUpdates() {
        daemonPollTimer.stop();
        checking = true;
        updating = false;
        imageUpdateDone = false;
        updateData = null;
        rebootRequired = false;
        pendingOpIsCheck = true;
        inQueueMode = false;
        completedUpdates = ({});
        backend.checkUpdates();
        pollTimer.start();
    }

    // Poll the op while a manual check (or update) is in progress
    Timer {
        id: pollTimer
        interval: 400
        repeat: true
        onTriggered: {
            backend.pollOp();
            if (!backend.opRunning) {
                pollTimer.stop();
                if (inQueueMode) {
                    var finishedStep = updateQueue[queueStep];
                    // Record batch completion for row hiding
                    if (backend.opResult === 1) {
                        var co = completedUpdates;
                        if (finishedStep === "packages") co["__packages__"] = true;
                        else if (finishedStep === "flatpak") co["__flatpak__"] = true;
                        completedUpdates = co;
                    }
                    queueStep++;
                    if (queueStep < updateQueue.length) {
                        _runQueueStep();
                    } else {
                        // Entire queue done
                        inQueueMode = false;
                        updateQueue = [];
                        updating = false;
                        currentOpLabel = "";
                        activeUpdateType = "";
                        if (finishedStep === "image") {
                            imageUpdateDone = true;
                        }
                        // Don't reload cache — rows already hidden via completedUpdates
                    }
                } else {
                    var savedType = activeUpdateType;
                    var wasImageUpdate = savedType === "image";
                    checking = false;
                    updating = false;
                    currentOpLabel = "";
                    activeUpdateType = "";
                    if (pendingOpIsCheck) {
                        if (backend.opResult === 1) {
                            try { updateData = JSON.parse(backend.readLog()); }
                            catch(e) { updateData = {}; }
                            rebootRequired = updateData && updateData.reboot_required === true;
                            backend.pendingUpdateCount = totalUpdates;
                        }
                    } else if (wasImageUpdate) {
                        imageUpdateDone = true;
                    } else if (backend.opResult === 1) {
                        // Individual or section update succeeded — record for row hiding
                        var co2 = completedUpdates;
                        if (savedType === "packages") {
                            co2["__packages__"] = true;
                        } else if (savedType === "flatpak") {
                            co2["__flatpak__"] = true;
                        } else if (savedType) {
                            co2[savedType] = true;
                        }
                        completedUpdates = co2;
                    }
                }
            }
        }
    }

    // Poll for the daemon cache file when the daemon is doing its first check
    Timer {
        id: daemonPollTimer
        interval: 3000
        repeat: true
        onTriggered: _loadFromCache()
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
                        if (imageUpdateDone) return "Reboot required to apply system upgrade";
                        if (updating) return currentOpLabel || "Updating…";
                        if (checking) return "Checking for updates…";
                        if (updateData === null) return "No update data yet";
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
                    visible: !checking && !updating && totalUpdates > 0 && !rebootRequired && !imageUpdateDone
                    highlighted: true
                    onClicked: _doUpdateAll()
                }

                Button {
                    visible: rebootRequired
                    highlighted: true
                    implicitWidth: 130; implicitHeight: 32
                    background: Rectangle { color: "#1976d2"; radius: 4 }
                    contentItem: Label { text: "🔄 Reboot Now"; color: "white"; font.pixelSize: 13; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                    onClicked: backend.rebootSystem()
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        // ── Content area ─────────────────────────────────────────────────────
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            // Daemon is running its first check
            Column {
                anchors.centerIn: parent
                spacing: 16
                visible: updateData === null && checking

                BusyIndicator { anchors.horizontalCenter: parent.horizontalCenter; running: true }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Checking for updates…"
                    color: root.dimText
                    font.pixelSize: 14
                }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "The background service is running its check"
                    color: root.dimText
                    font.pixelSize: 12
                }
            }

            // Image update complete — reboot required
            Column {
                anchors.centerIn: parent
                spacing: 20
                visible: imageUpdateDone

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "🔄"
                    font.pixelSize: 64
                }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "System Update Staged"
                    font.pixelSize: 20
                    font.bold: true
                }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "The new OS image is ready. Reboot to apply it."
                    font.pixelSize: 14
                    color: root.dimText
                }
                Button {
                    anchors.horizontalCenter: parent.horizontalCenter
                    highlighted: true
                    implicitWidth: 140; implicitHeight: 36
                    background: Rectangle { color: "#1976d2"; radius: 4 }
                    contentItem: Label {
                        text: "Reboot Now"
                        color: "white"
                        font.pixelSize: 14
                        font.bold: true
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: backend.rebootSystem()
                }
            }

            // Up to date
            Column {
                anchors.centerIn: parent
                spacing: 16
                visible: updateData !== null && totalUpdates === 0 && !rebootRequired && !imageUpdateDone

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
                    color: root.dimText
                }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Your system and all apps are up to date."
                    font.pixelSize: 12
                    color: root.dimText
                }
            }

            ScrollView {
                anchors.fill: parent
                contentWidth: availableWidth
                visible: updateData !== null && (totalUpdates > 0 || rebootRequired) && !imageUpdateDone
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
                                    color: root.dimText
                                    font.pixelSize: 12
                                }
                            }

                            Rectangle { width: parent.width; height: 1; color: palette.mid; opacity: 0.3 }

                            // Normal row: icon + info + button (hidden while image updating)
                            RowLayout {
                                width: parent.width
                                spacing: 10
                                visible: !(updating && activeUpdateType === "image")

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
                                            if (info.type === "switch" && info.available) {
                                                return (info.booted || "current") + "  →  " + info.available;
                                            }
                                            return "Refresh of " + (info.booted || "current");
                                        }
                                        color: root.dimText
                                        font.pixelSize: 11
                                    }
                                }

                                Button {
                                    id: imageUpdateBtn
                                    text: (updateData && updateData.image_info &&
                                           updateData.image_info.type === "switch") ? "Update" : "Apply Hotfix"
                                    visible: !rebootRequired
                                    onClicked: {
                                        if (!updateData || !updateData.image_info) return;
                                        var info = updateData.image_info;
                                        var utype = info.type || "upgrade";
                                        activeUpdateType = "image";
                                        inQueueMode = false;
                                        pendingOpIsCheck = false;
                                        updating = true;
                                        backend.upgradeImage(utype, info.repo || "", info.available || "");
                                        pollTimer.start();
                                    }
                                }
                            }

                            // In-progress state — shown instead of the button row
                            Column {
                                width: parent.width
                                spacing: 8
                                visible: updating && activeUpdateType === "image"

                                RowLayout {
                                    width: parent.width
                                    spacing: 10

                                    BusyIndicator {
                                        running: true
                                        implicitWidth: 24; implicitHeight: 24
                                    }

                                    Label {
                                        text: "Applying system update…"
                                        font.pixelSize: 13
                                        font.bold: true
                                        Layout.fillWidth: true
                                    }
                                }

                                ProgressBar {
                                    width: parent.width
                                    height: 10
                                    from: 0; to: 100
                                    indeterminate: backend.opProgress === 0
                                    value: backend.opProgress
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
                        title: "Runtimes/Add-ons"
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
                        title: "Overlay Dependencies"
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
        imageUpdateDone = false;
        updateQueue = [];
        if (updateData && updateData.packages && updateData.packages.length > 0)
            updateQueue.push("packages");
        if (updateData && updateData.flatpak && updateData.flatpak.length > 0)
            updateQueue.push("flatpak");
        if (updateData && updateData.image_available === true)
            updateQueue.push("image");
        if (updateQueue.length === 0) return;
        updating = true;
        inQueueMode = true;
        pendingOpIsCheck = false;
        queueStep = 0;
        _runQueueStep();
    }

    function _runQueueStep() {
        var step = updateQueue[queueStep];
        var total = updateQueue.length;
        var stepNum = queueStep + 1;
        if (step === "packages") {
            currentOpLabel = "Updating overlay packages… (" + stepNum + "/" + total + ")";
            activeUpdateType = "packages";
            backend.upgradePackages();
        } else if (step === "flatpak") {
            currentOpLabel = "Updating Flatpak apps… (" + stepNum + "/" + total + ")";
            activeUpdateType = "flatpak";
            backend.installApp("__upgrade_all__", "flatpak");
        } else if (step === "image") {
            currentOpLabel = "Updating system image… (" + stepNum + "/" + total + ")";
            activeUpdateType = "image";
            var info = updateData ? updateData.image_info : null;
            var utype = info ? (info.type || "upgrade") : "upgrade";
            var repo  = info ? (info.repo || "") : "";
            var tag   = info ? (info.available || "") : "";
            backend.upgradeImage(utype, repo, tag);
        }
        pollTimer.start();
    }

    function _doSectionUpdate(pkgs) {
        if (updating || checking) return;
        if (!pkgs || pkgs.length === 0) return;
        inQueueMode = false;
        pendingOpIsCheck = false;
        updating = true;
        var hasFlatpak = pkgs.some(function(p) { return p.pkg_type === "flatpak"; });
        var hasRpm     = pkgs.some(function(p) { return p.pkg_type === "rpm"; });
        if (hasFlatpak) {
            currentOpLabel = "Updating Flatpak apps…";
            activeUpdateType = "flatpak";
            backend.installApp("__upgrade_all__", "flatpak");
        } else if (hasRpm) {
            currentOpLabel = "Updating overlay packages…";
            activeUpdateType = "packages";
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
                    color: root.dimText
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

                    // Is this specific row's update currently running?
                    readonly property string _pkgId: modelData.app_id || modelData.id || modelData.name || ""
                    readonly property bool rowUpdating: {
                        if (!updatesPage.updating) return false;
                        var t = updatesPage.activeUpdateType;
                        if (!t) return false;
                        // All RPM rows light up when upgrading packages batch
                        if (t === "packages" && modelData.pkg_type === "rpm") return true;
                        // All flatpak rows light up when upgrading flatpak batch
                        if (t === "flatpak" && modelData.pkg_type === "flatpak") return true;
                        // Legacy type match
                        if (t === modelData.pkg_type) return true;
                        // Individual update: matched by id
                        if (_pkgId && t === _pkgId) return true;
                        return false;
                    }
                    // Has this row's update completed successfully?
                    readonly property bool rowCompleted: {
                        var cu = updatesPage.completedUpdates;
                        if (!cu) return false;
                        if (_pkgId && cu[_pkgId]) return true;
                        if (modelData.pkg_type === "rpm" && cu["__packages__"]) return true;
                        if (modelData.pkg_type === "flatpak" && cu["__flatpak__"]) return true;
                        return false;
                    }

                    visible: !rowCompleted

                    Rectangle {
                        width: parent.width
                        height: pkgCol.implicitHeight + 16
                        color: "transparent"

                        Column {
                            id: pkgCol
                            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: 4; rightMargin: 4 }
                            spacing: 4

                            RowLayout {
                                width: parent.width
                                spacing: 10

                                AppIcon {
                                    iconPath: {
                                        if (modelData.icon_path) return modelData.icon_path;
                                        var id = modelData.app_id || "";
                                        if (id && modelData.pkg_type === "flatpak")
                                            return "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/128x128/" + id + ".png";
                                        var name = (modelData.name || "").toLowerCase();
                                        if (name && modelData.pkg_type === "rpm")
                                            return "/usr/share/icons/hicolor/48x48/apps/" + name + ".png";
                                        return "";
                                    }
                                    iconUrl: modelData.icon_url || ""
                                    iconName: modelData.name || modelData.app_id || modelData.id || "?"
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
                                            color: root.dimText
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
                                    visible: !rowUpdating
                                    onClicked: {
                                        var pkg = modelData;
                                        var pkgId = pkg.app_id || pkg.id || pkg.name || "";
                                        updatesPage.inQueueMode = false;
                                        updatesPage.pendingOpIsCheck = false;
                                        updatesPage.updating = true;
                                        if (pkg.pkg_type === "flatpak") {
                                            // Individual flatpak update — track by specific app_id
                                            updatesPage.activeUpdateType = pkgId;
                                            backend.installApp(pkg.app_id || pkg.id || "", "flatpak");
                                        } else {
                                            // RPM individual update upgrades ALL packages — mark all rpm rows
                                            updatesPage.activeUpdateType = "packages";
                                            backend.upgradePackages();
                                        }
                                        pollTimer.start();
                                    }
                                }
                            }

                            // Per-row progress bar — visible while this row's update runs
                            ProgressBar {
                                width: parent.width
                                height: 6
                                visible: rowUpdating
                                from: 0; to: 100
                                indeterminate: backend.opProgress === 0
                                value: backend.opProgress
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
