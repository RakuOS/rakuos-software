import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
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
