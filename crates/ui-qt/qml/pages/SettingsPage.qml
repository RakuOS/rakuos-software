import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: settingsPage

    property var settings: ({
        update_interval: 1440,
        auto_check_packages: true,
        auto_check_flatpak: true,
        auto_check_image: true,
        auto_check_appimages: true,
        auto_update: false,
    })

    property bool saved: false

    function loadSettings() {
        try {
            var json = backend.loadSettings();
            var s = JSON.parse(json);
            settings = s;
            applySettings();
        } catch(e) {}
    }

    function applySettings() {
        intervalCombo.currentIndex = {
            360: 0, 720: 1, 1440: 2, 10080: 3, 0: 4
        }[settings.update_interval] || 2;

        checkPkgs.checked     = settings.auto_check_packages !== false;
        checkFlatpak.checked  = settings.auto_check_flatpak  !== false;
        checkImage.checked    = settings.auto_check_image     !== false;
        checkAI.checked       = settings.auto_check_appimages !== false;
        autoUpdate.checked    = settings.auto_update === true;
    }

    function saveSettings() {
        var s = {
            update_interval:      [360, 720, 1440, 10080, 0][intervalCombo.currentIndex] || 1440,
            auto_check_packages:  checkPkgs.checked,
            auto_check_flatpak:   checkFlatpak.checked,
            auto_check_image:     checkImage.checked,
            auto_check_appimages: checkAI.checked,
            auto_update:          autoUpdate.checked,
        };
        backend.saveSettings(JSON.stringify(s));
        saved = true;
        savedTimer.restart();
    }

    Timer {
        id: savedTimer
        interval: 2000
        onTriggered: saved = false
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        TabBar {
            id: settingsTab
            Layout.fillWidth: true

            TabButton { text: "🔄  Updates" }
            TabButton { text: "📦  Flatpak Repositories"; onClicked: flatpakTab.loadRemotes() }
            TabButton { text: "🔧  Firmware / LVFS";      onClicked: firmwareTab.loadData() }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: settingsTab.currentIndex

            // ── Updates tab ──────────────────────────────────────────────────
            ScrollView {
                contentWidth: availableWidth
                clip: true

                Column {
                    width: parent.width
                    spacing: 0
                    topPadding: 20
                    leftPadding: 24
                    rightPadding: 24
                    bottomPadding: 20

                    Label {
                        text: "Update Schedule"
                        font.pixelSize: 16
                        font.bold: true
                        bottomPadding: 4
                    }

                    Label {
                        text: "How often RakuOS Software checks for available updates in the background."
                        color: root.dimText
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        width: parent.width
                        bottomPadding: 16
                    }

                    RowLayout {
                        width: parent.width
                        spacing: 12

                        Label { text: "Check for updates:" }

                        ComboBox {
                            id: intervalCombo
                            model: ["Every 6 hours", "Every 12 hours", "Daily", "Weekly", "Manual only"]
                            currentIndex: 2
                            width: 200
                        }

                        Item { Layout.fillWidth: true }
                    }

                    Item { height: 16; width: 1 }
                    Rectangle { width: parent.width; height: 1; color: palette.mid; opacity: 0.3 }
                    Item { height: 16; width: 1 }

                    Label {
                        text: "Check for"
                        font.pixelSize: 14
                        font.bold: true
                        bottomPadding: 8
                    }

                    CheckBox { id: checkPkgs;    text: "Overlay package updates";       checked: true }
                    CheckBox { id: checkFlatpak; text: "Flatpak updates";               checked: true }
                    CheckBox { id: checkImage;   text: "System image updates (bootc)";  checked: true }
                    CheckBox { id: checkAI;      text: "AppImage updates";              checked: true }

                    Item { height: 16; width: 1 }
                    Rectangle { width: parent.width; height: 1; color: palette.mid; opacity: 0.3 }
                    Item { height: 16; width: 1 }

                    Label {
                        text: "Automatic Updates"
                        font.pixelSize: 14
                        font.bold: true
                        bottomPadding: 8
                    }

                    CheckBox {
                        id: autoUpdate
                        text: "Automatically install package and Flatpak updates when found"
                        checked: false
                    }

                    Label {
                        text: "System image updates always require manual approval and a reboot."
                        color: root.dimText
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        width: parent.width
                        topPadding: 4
                    }

                    Item { height: 24; width: 1 }

                    RowLayout {
                        width: parent.width
                        spacing: 12

                        Button {
                            text: "Save Settings"
                            highlighted: true
                            onClicked: saveSettings()
                        }

                        Label {
                            text: "✓ Settings saved"
                            color: "#4caf50"
                            font.pixelSize: 12
                            visible: saved
                        }

                        Item { Layout.fillWidth: true }
                    }
                }
            }

            // ── Flatpak repos tab ────────────────────────────────────────────
            Item {
                id: flatpakTab

                property var remotes: []
                property bool hasFlathub: false
                property string statusMsg: ""
                property bool statusOk: true

                function loadRemotes() {
                    try {
                        var json = backend.getFlatpakRemotes();
                        var data = JSON.parse(json);
                        remotes = data.remotes || [];
                        hasFlathub = data.has_flathub === true;
                    } catch(e) {
                        remotes = [];
                        hasFlathub = false;
                    }
                }

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0

                    // Header row
                    Rectangle {
                        Layout.fillWidth: true
                        height: hdrRow.implicitHeight + 24
                        color: palette.button

                        RowLayout {
                            id: hdrRow
                            anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                            spacing: 10

                            Column {
                                Layout.fillWidth: true
                                Label {
                                    text: "Flatpak Repositories"
                                    font.pixelSize: 15
                                    font.bold: true
                                }
                                Label {
                                    text: "Manage Flatpak repositories. System remotes are available to all users."
                                    font.pixelSize: 11
                                    color: root.dimText
                                    wrapMode: Text.WordWrap
                                    width: parent.width
                                }
                            }

                            Button {
                                text: "➕  Add Flathub"
                                visible: !flatpakTab.hasFlathub
                                onClicked: {
                                    var res = JSON.parse(backend.addFlathub(true));
                                    flatpakTab.statusMsg = res.msg || "";
                                    flatpakTab.statusOk  = res.ok === true;
                                    flatpakTab.loadRemotes();
                                }
                            }

                            Button {
                                text: "➕  Add Remote"
                                onClicked: addRemoteDlg.open()
                            }
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                    Label {
                        leftPadding: 16
                        topPadding: 8
                        bottomPadding: 4
                        text: flatpakTab.statusMsg
                        color: flatpakTab.statusOk ? "#4caf50" : "#e53935"
                        font.pixelSize: 12
                        visible: flatpakTab.statusMsg !== ""
                        Layout.fillWidth: true
                    }

                    ScrollView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: availableWidth
                        clip: true

                        Column {
                            width: parent.width
                            topPadding: 8
                            bottomPadding: 8
                            spacing: 8

                            // Empty state
                            Label {
                                anchors.horizontalCenter: parent.horizontalCenter
                                topPadding: 40
                                text: "No Flatpak remotes configured."
                                color: root.dimText
                                visible: flatpakTab.remotes.length === 0
                            }

                            Repeater {
                                model: flatpakTab.remotes

                                Rectangle {
                                    id: remoteRow
                                    width: parent.width - 32
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    height: remoteRowLayout.implicitHeight + 20
                                    radius: 6
                                    color: palette.button
                                    border.color: palette.mid
                                    border.width: 1

                                    property string rowStatusMsg: ""
                                    property bool rowStatusOk: true

                                    RowLayout {
                                        id: remoteRowLayout
                                        anchors { fill: parent; leftMargin: 14; rightMargin: 14 }
                                        spacing: 10

                                        CheckBox {
                                            checked: modelData.enabled
                                            onToggled: {
                                                var res = JSON.parse(backend.setFlatpakRemoteEnabled(
                                                    modelData.name, checked, modelData.system));
                                                remoteRow.rowStatusMsg = res.msg || "";
                                                remoteRow.rowStatusOk = res.ok === true;
                                                if (!res.ok) { checked = !checked; }
                                            }
                                        }

                                        Column {
                                            Layout.fillWidth: true
                                            spacing: 2

                                            RowLayout {
                                                spacing: 8
                                                Label {
                                                    text: modelData.title || modelData.name
                                                    font.bold: true
                                                    font.pixelSize: 13
                                                }
                                                Rectangle {
                                                    radius: 3
                                                    color: modelData.system ? "#1a237e" : "#37474f"
                                                    width: scopeLbl.implicitWidth + 8
                                                    height: 16
                                                    Label {
                                                        id: scopeLbl
                                                        anchors.centerIn: parent
                                                        text: modelData.system ? "system" : "user"
                                                        font.pixelSize: 9
                                                        color: "white"
                                                    }
                                                }
                                            }

                                            Label {
                                                text: modelData.url || ""
                                                font.pixelSize: 11
                                                color: root.dimText
                                                visible: text !== ""
                                                elide: Text.ElideRight
                                                width: parent.width
                                            }

                                            Label {
                                                text: remoteRow.rowStatusMsg
                                                font.pixelSize: 11
                                                color: remoteRow.rowStatusOk ? "#4caf50" : "#e53935"
                                                visible: text !== ""
                                            }
                                        }

                                        Button {
                                            flat: true
                                            implicitWidth: 72; implicitHeight: 30
                                            contentItem: Label {
                                                text: "Remove"
                                                color: "#e53935"
                                                font.pixelSize: 12
                                                horizontalAlignment: Text.AlignHCenter
                                                verticalAlignment: Text.AlignVCenter
                                            }
                                            onClicked: {
                                                var res = JSON.parse(backend.removeFlatpakRemote(
                                                    modelData.name, modelData.system));
                                                flatpakTab.statusMsg = res.msg || "";
                                                flatpakTab.statusOk  = res.ok === true;
                                                flatpakTab.loadRemotes();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Add remote dialog ─────────────────────────────────────────
                Dialog {
                    id: addRemoteDlg
                    title: "Add Flatpak Remote"
                    modal: true
                    width: 420
                    standardButtons: Dialog.Ok | Dialog.Cancel

                    onAccepted: {
                        var name = addNameField.text.trim();
                        var url  = addUrlField.text.trim();
                        if (!name || !url) {
                            addDlgStatus.text = "Name and URL are required.";
                            addDlgStatus.color = "#e53935";
                            return;
                        }
                        var res = JSON.parse(backend.addFlatpakRemote(name, url, addSystemChk.checked));
                        if (res.ok) {
                            flatpakTab.loadRemotes();
                            addNameField.text = "";
                            addUrlField.text  = "";
                        } else {
                            addDlgStatus.text  = res.msg || "Failed.";
                            addDlgStatus.color = "#e53935";
                        }
                    }

                    Column {
                        width: parent.width
                        spacing: 10

                        Label { text: "Name (e.g. flathub):" }
                        TextField {
                            id: addNameField
                            width: parent.width
                            placeholderText: "remote-name"
                        }

                        Label { text: "URL (.flatpakrepo or repository URL):" }
                        TextField {
                            id: addUrlField
                            width: parent.width
                            placeholderText: "https://…"
                        }

                        CheckBox {
                            id: addSystemChk
                            text: "Install as system-wide remote (recommended)"
                            checked: true
                        }

                        Label {
                            id: addDlgStatus
                            text: ""
                            wrapMode: Text.WordWrap
                            width: parent.width
                            visible: text !== ""
                        }
                    }
                }

                Component.onCompleted: loadRemotes()
            }

            // ── Firmware tab ─────────────────────────────────────────────────
            Item {
                id: firmwareTab

                property var firmwareData: null
                property bool refreshing: false
                property string refreshLog: ""
                property string statusMsg: ""
                property bool statusOk: true

                function loadData() {
                    try {
                        var json = backend.getFirmwareData();
                        firmwareData = JSON.parse(json);
                    } catch(e) {
                        firmwareData = { available: false, remotes: [], vendor_dirs: [], updates: [] };
                    }
                }

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0

                    // Header
                    Rectangle {
                        Layout.fillWidth: true
                        height: fwHdrRow.implicitHeight + 24
                        color: palette.button

                        RowLayout {
                            id: fwHdrRow
                            anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                            spacing: 10

                            Column {
                                Layout.fillWidth: true
                                Label {
                                    text: "Firmware & LVFS"
                                    font.pixelSize: 15
                                    font.bold: true
                                }
                                Label {
                                    text: "Manage firmware update sources via fwupd. LVFS provides updates from hardware vendors."
                                    font.pixelSize: 11
                                    color: root.dimText
                                    wrapMode: Text.WordWrap
                                    width: parent.width
                                }
                            }

                            BusyIndicator {
                                running: firmwareTab.refreshing
                                visible: firmwareTab.refreshing
                                implicitWidth: 22; implicitHeight: 22
                            }

                            Button {
                                text: "🔄  Refresh Metadata"
                                enabled: !firmwareTab.refreshing && (firmwareTab.firmwareData && firmwareTab.firmwareData.available)
                                onClicked: {
                                    firmwareTab.refreshing = true;
                                    firmwareTab.refreshLog = "";
                                    firmwareTab.statusMsg = "";
                                    backend.refreshFirmware();
                                    fwPollTimer.start();
                                }
                            }
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                    // Not available notice
                    Label {
                        leftPadding: 16
                        topPadding: 12
                        text: "⚠  fwupd is not installed. Install it to manage firmware updates."
                        color: "#ff9800"
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        visible: firmwareTab.firmwareData && !firmwareTab.firmwareData.available
                    }

                    Label {
                        leftPadding: 16
                        topPadding: 4
                        text: firmwareTab.statusMsg
                        color: firmwareTab.statusOk ? "#4caf50" : "#e53935"
                        font.pixelSize: 12
                        visible: firmwareTab.statusMsg !== ""
                        Layout.fillWidth: true
                    }

                    ScrollView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: availableWidth
                        clip: true
                        visible: firmwareTab.firmwareData && firmwareTab.firmwareData.available

                        Column {
                            width: parent.width
                            topPadding: 8
                            bottomPadding: 8
                            spacing: 8

                            Label {
                                leftPadding: 16
                                text: "Remotes"
                                font.pixelSize: 12
                                font.bold: true
                                color: root.dimText
                                visible: firmwareTab.firmwareData && firmwareTab.firmwareData.remotes &&
                                         firmwareTab.firmwareData.remotes.length > 0
                            }

                            // Empty state
                            Label {
                                anchors.horizontalCenter: parent.horizontalCenter
                                topPadding: 40
                                text: "No firmware remotes configured."
                                color: root.dimText
                                visible: !firmwareTab.firmwareData ||
                                         !firmwareTab.firmwareData.remotes ||
                                         firmwareTab.firmwareData.remotes.length === 0
                            }

                            Repeater {
                                model: (firmwareTab.firmwareData && firmwareTab.firmwareData.remotes)
                                       ? firmwareTab.firmwareData.remotes : []

                                Rectangle {
                                    id: fwRemoteRow
                                    width: parent.width - 32
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    height: fwRemoteLayout.implicitHeight + 20
                                    radius: 6
                                    color: palette.button
                                    border.color: palette.mid
                                    border.width: 1

                                    property string rowStatus: ""
                                    property bool rowOk: true

                                    RowLayout {
                                        id: fwRemoteLayout
                                        anchors { fill: parent; leftMargin: 14; rightMargin: 14 }
                                        spacing: 10

                                        CheckBox {
                                            checked: modelData.enabled
                                            onToggled: {
                                                var res = JSON.parse(backend.setFirmwareRemoteEnabled(
                                                    modelData.id, checked));
                                                fwRemoteRow.rowStatus = res.msg || "";
                                                fwRemoteRow.rowOk = res.ok === true;
                                                if (!res.ok) { checked = !checked; }
                                                else { firmwareTab.loadData(); }
                                            }
                                        }

                                        Column {
                                            Layout.fillWidth: true
                                            spacing: 2

                                            RowLayout {
                                                spacing: 8
                                                Label {
                                                    text: modelData.title || modelData.id
                                                    font.bold: true
                                                    font.pixelSize: 13
                                                }
                                                Label {
                                                    text: modelData.kind || ""
                                                    font.pixelSize: 10
                                                    color: root.dimText
                                                    visible: text !== ""
                                                }
                                            }

                                            Label {
                                                text: modelData.url || modelData.metadata_uri || ""
                                                font.pixelSize: 11
                                                color: root.dimText
                                                visible: text !== ""
                                                elide: Text.ElideRight
                                                width: parent.width
                                            }

                                            Label {
                                                text: modelData.last_updated ? "Updated: " + modelData.last_updated : ""
                                                font.pixelSize: 10
                                                color: root.dimText
                                                visible: text !== ""
                                            }

                                            Label {
                                                text: fwRemoteRow.rowStatus
                                                font.pixelSize: 11
                                                color: fwRemoteRow.rowOk ? "#4caf50" : "#e53935"
                                                visible: text !== ""
                                            }
                                        }
                                    }
                                }
                            }

                            // Vendor dirs section
                            Label {
                                leftPadding: 16
                                topPadding: 8
                                text: "Vendor Directories (" +
                                      ((firmwareTab.firmwareData && firmwareTab.firmwareData.vendor_dirs)
                                       ? firmwareTab.firmwareData.vendor_dirs.length : 0) + " found)"
                                font.pixelSize: 12
                                font.bold: true
                                color: root.dimText
                                visible: firmwareTab.firmwareData && firmwareTab.firmwareData.vendor_dirs &&
                                         firmwareTab.firmwareData.vendor_dirs.length > 0
                            }

                            Repeater {
                                model: (firmwareTab.firmwareData && firmwareTab.firmwareData.vendor_dirs)
                                       ? firmwareTab.firmwareData.vendor_dirs : []

                                Rectangle {
                                    width: parent.width - 32
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    height: vdLayout.implicitHeight + 16
                                    radius: 6
                                    color: palette.button
                                    border.color: palette.mid
                                    border.width: 1
                                    opacity: 0.8

                                    RowLayout {
                                        id: vdLayout
                                        anchors { fill: parent; leftMargin: 14; rightMargin: 14 }
                                        spacing: 10

                                        Column {
                                            Layout.fillWidth: true
                                            spacing: 2
                                            Label {
                                                text: modelData.title || modelData.id
                                                font.bold: true
                                                font.pixelSize: 12
                                            }
                                            Label {
                                                text: modelData.filename || ""
                                                font.pixelSize: 10
                                                color: root.dimText
                                                visible: text !== ""
                                                elide: Text.ElideRight
                                                width: parent.width
                                            }
                                        }
                                    }
                                }
                            }

                            // Refresh log
                            Rectangle {
                                width: parent.width - 32
                                anchors.horizontalCenter: parent.horizontalCenter
                                height: 100
                                color: Qt.rgba(0, 0, 0, 0.75)
                                radius: 4
                                clip: true
                                visible: firmwareTab.refreshing || firmwareTab.refreshLog !== ""

                                ScrollView {
                                    anchors.fill: parent
                                    contentWidth: availableWidth
                                    Label {
                                        width: parent.width
                                        padding: 8
                                        text: firmwareTab.refreshLog
                                        color: "#d0d0d0"
                                        font.family: "monospace"
                                        font.pixelSize: 11
                                        wrapMode: Text.WordWrap
                                    }
                                }
                            }
                        }
                    }
                }

                Timer {
                    id: fwPollTimer
                    interval: 300
                    repeat: true
                    onTriggered: {
                        backend.pollOp();
                        firmwareTab.refreshLog = backend.readLog();
                        if (!backend.opRunning) {
                            stop();
                            firmwareTab.refreshing = false;
                            if (backend.opResult === 1) {
                                firmwareTab.statusMsg = "✓ Firmware metadata refreshed.";
                                firmwareTab.statusOk  = true;
                                firmwareTab.loadData();
                            } else {
                                firmwareTab.statusMsg = "✗ Refresh failed.";
                                firmwareTab.statusOk  = false;
                            }
                        }
                    }
                }

                Component.onCompleted: loadData()
            }
        }
    }

    Component.onCompleted: loadSettings()
}
