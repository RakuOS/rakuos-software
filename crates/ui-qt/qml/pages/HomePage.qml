import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "../components"

Item {
    id: homePage

    // Called when this page becomes current
    function activate() {
        if (homeData === null) {
            loadData();
        }
    }

    property var homeData: null
    property bool loading: false

    function loadData() {
        loading = true;
        homeData = null;
        backend.loadHome();
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
                loading = false;
                if (backend.opResult === 1) {
                    var json = backend.homeDataLoaded();
                    try {
                        homeData = JSON.parse(json);
                    } catch(e) {
                        homeData = {};
                    }
                }
            }
        }
    }

    // ── Section builder from JSON array ─────────────────────────────────────
    function appsForSection(key) {
        if (!homeData) return [];
        var arr = homeData[key];
        if (!arr) return [];
        return arr;
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        Column {
            width: parent.width
            spacing: 0
            topPadding: 20
            bottomPadding: 20

            // Loading / error state
            Item {
                width: parent.width
                height: 60
                visible: loading

                Row {
                    anchors.centerIn: parent
                    spacing: 12
                    BusyIndicator { running: loading; implicitWidth: 28; implicitHeight: 28 }
                    Label { text: "Loading home page…"; anchors.verticalCenter: parent.verticalCenter }
                }
            }

            // Sections
            Repeater {
                model: [
                    { key: "picks",   title: "Editor's Picks"  },
                    { key: "popular", title: "Popular Apps"     },
                    { key: "updated", title: "Recently Updated" },
                    { key: "new",     title: "New Apps"         },
                ]

                Column {
                    width: homePage.width
                    spacing: 0
                    visible: !loading && homeData !== null && appsForSection(modelData.key).length > 0

                    // Section header
                    Item {
                        width: parent.width
                        height: 44

                        Label {
                            anchors { left: parent.left; leftMargin: 24; verticalCenter: parent.verticalCenter }
                            text: modelData.title
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Horizontal scroll row of app cards
                    ScrollView {
                        width: parent.width
                        height: 130
                        contentWidth: appRow.implicitWidth
                        contentHeight: height
                        ScrollBar.vertical.policy: ScrollBar.AlwaysOff
                        ScrollBar.horizontal.policy: ScrollBar.AsNeeded
                        clip: true

                        Row {
                            id: appRow
                            spacing: 12
                            anchors.verticalCenter: parent.verticalCenter

                            Item { width: 24; height: 1 }

                            Repeater {
                                model: {
                                    var arr = appsForSection(modelData.key);
                                    return arr.slice(0, 12);
                                }

                                Rectangle {
                                    width: 110
                                    height: 110
                                    radius: 10
                                    color: palette.button
                                    border.color: palette.mid
                                    border.width: 1

                                    Column {
                                        anchors { fill: parent; margins: 8 }
                                        spacing: 4

                                        AppIcon {
                                            iconPath: modelData.icon_path || ""
                                            iconName: modelData.name || modelData.id || "?"
                                            size: 48
                                            anchors.horizontalCenter: parent.horizontalCenter
                                        }

                                        Label {
                                            width: parent.width
                                            text: modelData.name || modelData.id || ""
                                            font.pixelSize: 11
                                            elide: Text.ElideRight
                                            horizontalAlignment: Text.AlignHCenter
                                        }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.showDetail(modelData)
                                    }
                                }
                            }
                            Item { width: 24; height: 1 }
                        }
                    }

                    Item { width: parent.width; height: 16 }
                }
            }

            // Empty state (loaded but no data)
            Item {
                width: parent.width
                height: 200
                visible: !loading && homeData !== null &&
                         appsForSection("picks").length === 0 &&
                         appsForSection("popular").length === 0

                Column {
                    anchors.centerIn: parent
                    spacing: 12

                    Label {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Could not load home page data."
                        color: palette.mid
                    }

                    Button {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Retry"
                        onClicked: loadData()
                    }
                }
            }
        }
    }

    Component.onCompleted: {
        activate();
    }
}
