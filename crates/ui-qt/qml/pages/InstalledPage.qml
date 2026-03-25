import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "../components"

Item {
    id: installedPage

    property var allApps: []
    property bool loading: false

    // Filtered lists by source
    property var rpmApps: []
    property var flatpakApps: []
    property var appImages: []
    property var webApps: []

    function activate() {
        loadInstalled();
    }

    function loadInstalled() {
        loading = true;
        allApps = [];
        rpmApps = []; flatpakApps = []; appImages = []; webApps = [];
        backend.loadInstalled();
        pollTimer.start();
    }

    function filterApps(arr) {
        var rpm = [], fp = [], ai = [], wa = [];
        for (var i = 0; i < arr.length; i++) {
            var a = arr[i];
            var src = a.source || "";
            if (src === "flatpak")   fp.push(a);
            else if (src === "appimage") ai.push(a);
            else if (src === "webapp")   wa.push(a);
            else                         rpm.push(a);
        }
        // Sort alphabetically
        function byName(x, y) {
            var xn = (x.name || x.id || "").toLowerCase();
            var yn = (y.name || y.id || "").toLowerCase();
            return xn < yn ? -1 : xn > yn ? 1 : 0;
        }
        rpmApps     = rpm.sort(byName);
        flatpakApps = fp.sort(byName);
        appImages   = ai.sort(byName);
        webApps     = wa.sort(byName);
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
                    try { allApps = JSON.parse(backend.readLog()); }
                    catch(e) { allApps = []; }
                    filterApps(allApps);
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── Tab bar ─────────────────────────────────────────────────────────
        TabBar {
            id: tabBar
            Layout.fillWidth: true

            TabButton {
                text: "RPM (" + installedPage.rpmApps.length + ")"
            }
            TabButton {
                text: "Flatpak (" + installedPage.flatpakApps.length + ")"
            }
            TabButton {
                text: "AppImages (" + installedPage.appImages.length + ")"
            }
            TabButton {
                text: "Web Apps (" + installedPage.webApps.length + ")"
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

        // ── Loading spinner ──────────────────────────────────────────────────
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: loading

            Row {
                anchors.centerIn: parent
                spacing: 12
                BusyIndicator { running: loading; implicitWidth: 32; implicitHeight: 32 }
                Label { text: "Loading installed apps…"; anchors.verticalCenter: parent.verticalCenter }
            }
        }

        // ── Tab content ─────────────────────────────────────────────────────
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabBar.currentIndex
            visible: !loading

            // RPM tab
            AppListView {
                apps: installedPage.rpmApps
                emptyText: "No overlay packages installed."
                onRemoveApp: backend.removeApp(appId, "native")
                onReloadRequested: installedPage.loadInstalled()
            }

            // Flatpak tab
            AppListView {
                apps: installedPage.flatpakApps
                emptyText: "No Flatpaks installed."
                onRemoveApp: backend.removeApp(appId, "flatpak")
                onReloadRequested: installedPage.loadInstalled()
            }

            // AppImages tab
            AppListView {
                apps: installedPage.appImages
                emptyText: "No AppImages installed."
                onRemoveApp: backend.removeApp(appId, "appimage")
                onReloadRequested: installedPage.loadInstalled()
            }

            // Web Apps tab
            AppListView {
                apps: installedPage.webApps
                emptyText: "No web apps installed."
                onRemoveApp: backend.removeApp(appId, "webapp")
                onReloadRequested: installedPage.loadInstalled()
            }
        }
    }



    // ── Reusable list view component ──────────────────────────────────────────
    component AppListView: Item {
        id: listView
        property var apps: []
        property string emptyText: "Nothing installed."
        signal removeApp(string appId)
        signal reloadRequested()

        Label {
            anchors.centerIn: parent
            text: listView.emptyText
            color: palette.mid
            visible: listView.apps.length === 0
        }

        ScrollView {
            anchors.fill: parent
            contentWidth: availableWidth
            visible: listView.apps.length > 0
            clip: true

            Column {
                width: parent.width
                topPadding: 4
                bottomPadding: 8

                Repeater {
                    model: listView.apps

                    Rectangle {
                        id: appRowRect
                        width: listView.width
                        height: 52
                        color: "transparent"

                        property bool removing: false
                        property bool removed: false
                        visible: !removed

                        RowLayout {
                            anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                            spacing: 12

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
                                    font.bold: true
                                    elide: Text.ElideRight
                                    width: parent.width
                                }

                                Label {
                                    text: (modelData.summary || modelData.description || "").substring(0, 80) +
                                          ((modelData.summary || "").length > 80 ? "…" : "")
                                    font.pixelSize: 11
                                    color: palette.mid
                                    visible: text !== ""
                                    elide: Text.ElideRight
                                    width: parent.width
                                }
                            }

                            Label {
                                text: modelData.version || ""
                                font.pixelSize: 11
                                color: palette.mid
                                visible: text !== ""
                            }

                            // Uninstall button / busy / result
                            Button {
                                text: "Uninstall"
                                visible: !appRowRect.removing
                                flat: true
                                contentItem: Label {
                                    text: "Uninstall"
                                    color: "#e53935"
                                    font.pixelSize: 12
                                }
                                onClicked: {
                                    appRowRect.removing = true;
                                    listView.removeApp(modelData.id || "");
                                    // Watch for op completion
                                    removeWatcher.start();
                                }
                            }

                            BusyIndicator {
                                running: appRowRect.removing
                                visible: appRowRect.removing
                                implicitWidth: 20; implicitHeight: 20
                            }
                        }

                        Rectangle {
                            anchors { bottom: parent.bottom; left: parent.left; right: parent.right; leftMargin: 16; rightMargin: 16 }
                            height: 1
                            color: palette.mid
                            opacity: 0.15
                        }

                        Timer {
                            id: removeWatcher
                            interval: 400
                            repeat: true
                            onTriggered: {
                                backend.pollOp();
                                if (!backend.opRunning) {
                                    stop();
                                    appRowRect.removing = false;
                                    if (backend.opResult === 1) {
                                        appRowRect.removed = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
