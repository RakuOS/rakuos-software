import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtQuick.Dialogs
import "../components"

Item {
    id: webAppsPage

    property var apps: []
    property bool loading: false

    function activate() {
        loading = true;
        apps = [];
        backend.loadWebAppCatalog();
        pollTimer.start();
    }

    // Returns true if the icon_path is actually an HTTP(S) URL
    function isUrl(s) { return s && (s.startsWith("http://") || s.startsWith("https://")); }

    Timer {
        id: pollTimer
        interval: 300
        repeat: true
        onTriggered: {
            backend.pollOp();
            if (!backend.opRunning) {
                pollTimer.stop();
                loading = false;
                if (backend.opResult === 1) {
                    try { apps = JSON.parse(backend.readLog()); }
                    catch(e) { apps = []; }
                }
            }
        }
    }

    // ── Custom webapp install polling ─────────────────────────────────────────
    Timer {
        id: customInstallTimer
        interval: 300
        repeat: true
        onTriggered: {
            backend.pollOp();
            if (!backend.opRunning) {
                customInstallTimer.stop();
                customDialog.installing = false;
                if (backend.opResult === 1) {
                    customDialog.close();
                    // Reload full catalog to pick up the new custom app
                    webAppsPage.activate();
                } else {
                    customDialog.errorText = "Failed to add web app. Check the URL and try again.";
                }
            }
        }
    }

    // ── File dialog for browsing local icons ──────────────────────────────────
    FileDialog {
        id: iconFileDialog
        title: "Choose Icon"
        nameFilters: ["Image files (*.png *.jpg *.jpeg *.svg *.ico *.webp)", "All files (*)"]
        onAccepted: {
            var path = selectedFile.toString();
            // Strip file:// prefix
            if (path.startsWith("file://")) path = path.substring(7);
            customDialog.localIconPath = path;
            customDialog.localIconName = path.split("/").pop();
        }
    }

    // ── Custom webapp dialog ──────────────────────────────────────────────────
    Popup {
        id: customDialog
        anchors.centerIn: parent
        width: Math.min(parent.width * 0.9, 520)
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape

        // Dialog state
        property bool   installing:    false
        property string errorText:     ""
        property string localIconPath: ""
        property string localIconName: ""

        function clear() {
            nameField.text     = "";
            urlField.text      = "";
            descField.text     = "";
            iconUrlField.text  = "";
            catCombo.currentIndex = 0;
            localIconPath = "";
            localIconName = "";
            errorText     = "";
            installing    = false;
            localRadio.checked = true;
        }

        onOpened: clear()

        background: Rectangle {
            color: palette.window
            border.color: palette.mid
            border.width: 1
            radius: 8
        }

        contentItem: ColumnLayout {
            spacing: 0

            // ── Dialog header ─────────────────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                height: 48
                color: Qt.darker(palette.window, 1.05)
                radius: 8

                // Square off bottom corners
                Rectangle {
                    anchors { bottom: parent.bottom; left: parent.left; right: parent.right }
                    height: 8
                    color: parent.color
                }

                Label {
                    anchors.centerIn: parent
                    text: "Add Custom Web App"
                    font.pixelSize: 15
                    font.bold: true
                }
            }

            // ── Form ──────────────────────────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                Layout.margins: 16
                spacing: 10

                // Name
                ColumnLayout {
                    spacing: 4
                    Layout.fillWidth: true
                    Label { text: "App Name *"; font.pixelSize: 12; font.bold: true }
                    TextField {
                        id: nameField
                        Layout.fillWidth: true
                        placeholderText: "My App"
                        enabled: !customDialog.installing
                    }
                }

                // URL
                ColumnLayout {
                    spacing: 4
                    Layout.fillWidth: true
                    Label { text: "URL *"; font.pixelSize: 12; font.bold: true }
                    TextField {
                        id: urlField
                        Layout.fillWidth: true
                        placeholderText: "https://example.com"
                        enabled: !customDialog.installing
                    }
                }

                // Description
                ColumnLayout {
                    spacing: 4
                    Layout.fillWidth: true
                    Label { text: "Description"; font.pixelSize: 12; font.bold: true }
                    TextField {
                        id: descField
                        Layout.fillWidth: true
                        placeholderText: "Optional description"
                        enabled: !customDialog.installing
                    }
                }

                // Category
                ColumnLayout {
                    spacing: 4
                    Layout.fillWidth: true
                    Label { text: "Menu Category"; font.pixelSize: 12; font.bold: true }
                    ComboBox {
                        id: catCombo
                        Layout.fillWidth: true
                        model: ["Network", "Office", "Utility", "AudioVideo",
                                "Education", "Game", "Graphics", "Science"]
                        enabled: !customDialog.installing
                    }
                }

                // Icon section
                ColumnLayout {
                    spacing: 6
                    Layout.fillWidth: true

                    Label { text: "Icon"; font.pixelSize: 12; font.bold: true }

                    // Radio toggle: Local File / From URL
                    RowLayout {
                        spacing: 16
                        RadioButton {
                            id: localRadio
                            text: "Local File"
                            checked: true
                            enabled: !customDialog.installing
                        }
                        RadioButton {
                            id: urlRadio
                            text: "From URL"
                            enabled: !customDialog.installing
                        }
                    }

                    // Local file row
                    RowLayout {
                        visible: localRadio.checked
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: customDialog.localIconName !== ""
                                  ? customDialog.localIconName
                                  : "No file chosen"
                            color: customDialog.localIconName !== ""
                                   ? palette.windowText
                                   : root.dimText
                            Layout.fillWidth: true
                            elide: Text.ElideMiddle
                        }

                        Button {
                            text: "Browse…"
                            enabled: !customDialog.installing
                            onClicked: iconFileDialog.open()
                        }
                    }

                    // URL icon row
                    TextField {
                        id: iconUrlField
                        visible: urlRadio.checked
                        Layout.fillWidth: true
                        placeholderText: "https://example.com/icon.png"
                        enabled: !customDialog.installing
                    }
                }

                // Error message
                Label {
                    visible: customDialog.errorText !== ""
                    text: customDialog.errorText
                    color: "#cc0000"
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                    font.pixelSize: 12
                }
            }

            // ── Buttons ───────────────────────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                height: 1
                color: palette.mid
                opacity: 0.4
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 12
                spacing: 8

                Item { Layout.fillWidth: true }

                Button {
                    text: "Cancel"
                    enabled: !customDialog.installing
                    onClicked: customDialog.close()
                }

                Button {
                    text: customDialog.installing ? "Adding…" : "Add Web App"
                    enabled: !customDialog.installing
                    highlighted: true
                    onClicked: {
                        if (nameField.text.trim() === "") {
                            customDialog.errorText = "App name is required.";
                            return;
                        }
                        if (urlField.text.trim() === "") {
                            customDialog.errorText = "URL is required.";
                            return;
                        }
                        customDialog.errorText  = "";
                        customDialog.installing = true;

                        var iconSource = urlRadio.checked
                            ? iconUrlField.text.trim()
                            : customDialog.localIconPath;

                        backend.installCustomWebApp(
                            nameField.text.trim(),
                            urlField.text.trim(),
                            descField.text.trim(),
                            catCombo.currentText,
                            iconSource
                        );
                        customInstallTimer.start();
                    }
                }
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── Header ────────────────────────────────────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            height: 48
            color: palette.button

            RowLayout {
                anchors { fill: parent; leftMargin: 16; rightMargin: 16 }

                Label {
                    text: "Web Apps"
                    font.pixelSize: 16
                    font.bold: true
                    Layout.fillWidth: true
                }

                Label {
                    text: webAppsPage.apps.length > 0
                          ? webAppsPage.apps.length + " available"
                          : ""
                    font.pixelSize: 12
                    color: root.dimText
                }

                Button {
                    text: "+ Add Custom"
                    highlighted: true
                    onClicked: customDialog.open()
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        // ── Loading spinner ───────────────────────────────────────────────────
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: loading

            Row {
                anchors.centerIn: parent
                spacing: 12
                BusyIndicator { running: loading; implicitWidth: 32; implicitHeight: 32 }
                Label { text: "Loading web apps…"; anchors.verticalCenter: parent.verticalCenter }
            }
        }

        // ── App grid ──────────────────────────────────────────────────────────
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            visible: !loading

            Column {
                width: parent.width
                topPadding: 12
                bottomPadding: 12

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    topPadding: 40
                    text: "No web apps available."
                    color: root.dimText
                    visible: webAppsPage.apps.length === 0
                }

                // Wrap cards in a Flow so they reflow with window width
                Flow {
                    id: appGrid
                    width: parent.width
                    leftPadding: 12
                    rightPadding: 12
                    spacing: 10

                    Repeater {
                        model: webAppsPage.apps

                        Rectangle {
                            id: card
                            width: 130
                            height: 150
                            radius: 8
                            color: cardArea.containsMouse
                                ? Qt.rgba(palette.highlight.r, palette.highlight.g, palette.highlight.b, 0.10)
                                : palette.base
                            border.color: palette.mid
                            border.width: 1

                            Column {
                                anchors {
                                    fill: parent
                                    topMargin: 14
                                    bottomMargin: 10
                                    leftMargin: 8
                                    rightMargin: 8
                                }
                                spacing: 6

                                // Icon
                                AppIcon {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    iconPath: !webAppsPage.isUrl(modelData.icon_path) ? (modelData.icon_path || "") : ""
                                    iconUrl:  webAppsPage.isUrl(modelData.icon_path) ? modelData.icon_path
                                              : (webAppsPage.isUrl(modelData.icon_url) ? modelData.icon_url : "")
                                    iconName: modelData.name || modelData.id || "?"
                                    size: 56
                                }

                                // App name
                                Label {
                                    width: parent.width
                                    text: modelData.name || modelData.id || ""
                                    font.bold: true
                                    font.pixelSize: 12
                                    wrapMode: Text.WordWrap
                                    horizontalAlignment: Text.AlignHCenter
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                }

                                // Installed badge
                                Rectangle {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    visible: modelData.installed === true
                                    radius: 4
                                    color: root.sourceColor("webapp")
                                    width: installedLbl.implicitWidth + 10
                                    height: installedLbl.implicitHeight + 4

                                    Label {
                                        id: installedLbl
                                        anchors.centerIn: parent
                                        text: "Installed"
                                        font.pixelSize: 9
                                        color: "white"
                                    }
                                }
                            }

                            MouseArea {
                                id: cardArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.showDetail(modelData)
                            }
                        }
                    }
                }
            }
        }
    }
}
