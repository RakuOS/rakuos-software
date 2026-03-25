import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "../components"

Item {
    id: detailPage

    property var app: null
    signal backRequested()

    function loadApp(appData) {
        app = appData;
        screenshotIndex = 0;
        screenshotModel.clear();
        if (app && app.screenshots) {
            for (var i = 0; i < Math.min(app.screenshots.length, 8); i++) {
                screenshotModel.append({ url: app.screenshots[i] });
            }
        }
        if (sourceSelector.visible) {
            sourceSelector.currentIndex = 0;
        }

        // If this is partial data (HomeApp from home/search card has no
        // 'installed' field), fetch the full record in the background.
        if (app && app.id && typeof app.installed === 'undefined') {
            backend.loadAppById(app.id);
            detailFetchTimer.start();
        }
    }

    // Async full-detail fetch — fires when detail page receives partial data
    Timer {
        id: detailFetchTimer
        interval: 300
        repeat: true
        onTriggered: {
            backend.pollOp();
            if (!backend.opRunning) {
                detailFetchTimer.stop();
                if (backend.opResult === 1) {
                    var json = backend.readLog();
                    try {
                        var fullApp = JSON.parse(json);
                        if (fullApp && fullApp.id) {
                            detailPage.app = fullApp;
                            if (fullApp.screenshots && fullApp.screenshots.length > 0
                                    && screenshotModel.count === 0) {
                                for (var i = 0; i < Math.min(fullApp.screenshots.length, 8); i++) {
                                    screenshotModel.append({ url: fullApp.screenshots[i] });
                                }
                            }
                            if (sourceSelector.visible) {
                                sourceSelector.currentIndex = 0;
                            }
                        }
                    } catch(e) {}
                }
            }
        }
    }

    // Returns the currently selected source object
    function selectedSource() {
        if (!app) return null;
        if (Array.isArray(app.sources) && app.sources.length > 0) {
            var idx = sourceSelector.currentIndex;
            return app.sources[idx] || app.sources[0];
        }
        return app;
    }

    property int screenshotIndex: 0

    ListModel { id: screenshotModel }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── Top bar ───────────────────────────────────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            height: 48
            color: palette.button

            RowLayout {
                anchors { fill: parent; leftMargin: 12; rightMargin: 12 }
                spacing: 8

                Button {
                    text: "← Back"
                    flat: true
                    onClicked: detailPage.backRequested()
                }

                Label {
                    text: app ? (app.name || "") : ""
                    font.pixelSize: 15
                    font.bold: true
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                // Source selector — shown when both native and Flatpak are available
                ComboBox {
                    id: sourceSelector
                    visible: app != null && Array.isArray(app.sources) && app.sources.length > 1
                    width: 160
                    model: {
                        if (!app || !Array.isArray(app.sources)) return [];
                        return app.sources.map(function(s) {
                            return s.label + (s.installed ? " ✓" : "");
                        });
                    }
                }

                // Install / Remove button
                Button {
                    id: installBtn
                    visible: app != null
                    property var src: detailPage.selectedSource()
                    Connections {
                        target: sourceSelector
                        function onCurrentIndexChanged() { installBtn.src = detailPage.selectedSource(); }
                    }
                    text: src != null && src.installed === true ? "Remove" : "Install"
                    highlighted: src != null && src.installed !== true
                    onClicked: {
                        var s = detailPage.selectedSource();
                        if (!s) return;
                        if (s.installed) {
                            backend.removeApp(s.id || "", s.source || "");
                        } else {
                            backend.installApp(s.id || "", s.source || "");
                        }
                    }
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        // ── Scroll content ────────────────────────────────────────────────────
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            Column {
                width: parent.width
                topPadding: 24
                bottomPadding: 24
                spacing: 0

                // ── Hero ──────────────────────────────────────────────────────
                Item {
                    width: parent.width
                    height: heroRow.implicitHeight + 32
                    clip: false

                    RowLayout {
                        id: heroRow
                        anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: 28; rightMargin: 28; topMargin: 8 }
                        spacing: 20

                        AppIcon {
                            iconPath: app ? (app.icon_path || "") : ""
                            iconName: app ? (app.name || app.id || "?") : "?"
                            size: 80
                            Layout.alignment: Qt.AlignTop
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop
                            spacing: 4

                            Label {
                                text: app ? (app.name || app.id || "") : ""
                                font.pixelSize: 22
                                font.bold: true
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            Label {
                                text: app ? (app.summary || "") : ""
                                font.pixelSize: 13
                                color: palette.mid
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                                visible: text !== ""
                            }

                            Label {
                                text: app ? (app.developer || "") : ""
                                font.pixelSize: 11
                                color: palette.mid
                                visible: text !== ""
                            }

                            // Meta row: source badge, version, license
                            RowLayout {
                                spacing: 12
                                Layout.topMargin: 4

                                Rectangle {
                                    visible: app != null && (app.source || "") !== ""
                                    radius: 4
                                    color: {
                                        if (!app) return palette.button;
                                        if (app.source === "flatpak") return "#1565c0";
                                        if (app.source === "terra") return "#2e7d32";
                                        return "#37474f";
                                    }
                                    width: sourceLbl.implicitWidth + 12
                                    height: sourceLbl.implicitHeight + 6

                                    Label {
                                        id: sourceLbl
                                        anchors.centerIn: parent
                                        text: app ? (app.source || "") : ""
                                        font.pixelSize: 10
                                        color: "white"
                                    }
                                }

                                Label {
                                    text: app && app.version ? "v" + app.version : ""
                                    font.pixelSize: 11
                                    color: palette.mid
                                    visible: text !== ""
                                }

                                Label {
                                    text: app && app.license ? app.license : ""
                                    font.pixelSize: 11
                                    color: palette.mid
                                    visible: text !== ""
                                }
                            }
                        }
                    }
                }

                // Separator
                Rectangle {
                    width: parent.width - 56
                    height: 1
                    anchors.horizontalCenter: parent.horizontalCenter
                    color: palette.mid
                    opacity: 0.2
                }

                Item { width: 1; height: 20 }

                // ── Screenshot carousel ───────────────────────────────────────
                Item {
                    width: parent.width
                    height: screenshotModel.count > 0 ? 320 : 0
                    visible: screenshotModel.count > 0

                    Image {
                        id: mainShot
                        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 28 }
                        height: 280
                        source: screenshotModel.count > 0 ? screenshotModel.get(detailPage.screenshotIndex).url : ""
                        fillMode: Image.PreserveAspectFit
                        smooth: true
                        clip: true
                    }

                    Button {
                        anchors { left: parent.left; verticalCenter: mainShot.verticalCenter; leftMargin: 34 }
                        text: "‹"
                        visible: screenshotModel.count > 1
                        enabled: detailPage.screenshotIndex > 0
                        onClicked: detailPage.screenshotIndex--
                        width: 44; height: 44
                        flat: true
                        background: Rectangle {
                            radius: 22
                            color: parent.enabled ? "rgba(0,0,0,0.5)" : "rgba(0,0,0,0.2)"
                        }
                    }

                    Button {
                        anchors { right: parent.right; verticalCenter: mainShot.verticalCenter; rightMargin: 34 }
                        text: "›"
                        visible: screenshotModel.count > 1
                        enabled: detailPage.screenshotIndex < screenshotModel.count - 1
                        onClicked: detailPage.screenshotIndex++
                        width: 44; height: 44
                        flat: true
                        background: Rectangle {
                            radius: 22
                            color: parent.enabled ? "rgba(0,0,0,0.5)" : "rgba(0,0,0,0.2)"
                        }
                    }

                    Row {
                        anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter }
                        spacing: 8
                        Repeater {
                            model: screenshotModel.count
                            Label {
                                text: "●"
                                font.pixelSize: 10
                                color: index === detailPage.screenshotIndex ? palette.highlight : palette.mid
                            }
                        }
                    }
                }

                Item { width: 1; height: 20 }

                // ── Description ───────────────────────────────────────────────
                Column {
                    width: parent.width - 56
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 8
                    visible: app != null && (app.description || "") !== ""

                    Label {
                        text: "About this app"
                        font.pixelSize: 15
                        font.bold: true
                    }

                    Label {
                        text: app ? (app.description || "") : ""
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        width: parent.width
                        color: palette.text
                    }
                }

                Item { width: 1; height: 24 }

                // ── Info cards ────────────────────────────────────────────────
                Row {
                    width: parent.width - 56
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 12
                    visible: app != null

                    Repeater {
                        model: {
                            if (!app) return [];
                            var cards = [];
                            if (app.url_homepage) cards.push({ label: "Website", value: app.url_homepage });
                            if (app.package_name) cards.push({ label: "Package", value: app.package_name });
                            if (app.developer)    cards.push({ label: "Developer", value: app.developer });
                            return cards;
                        }

                        Rectangle {
                            width: 160
                            height: 60
                            radius: 8
                            color: palette.button
                            border.color: palette.mid
                            border.width: 1

                            Column {
                                anchors { fill: parent; margins: 10 }
                                spacing: 4
                                Label { text: modelData.label; font.pixelSize: 10; color: palette.mid }
                                Label { text: modelData.value; font.pixelSize: 11; elide: Text.ElideRight; width: parent.width }
                            }
                        }
                    }
                }
            }
        }
    }
}
