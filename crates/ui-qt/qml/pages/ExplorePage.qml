import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "../components"

Item {
    id: explorePage

    // ── State ──────────────────────────────────────────────────────────────────
    property string selectedCategory: ""
    property string selectedLabel: ""
    property var    subcatList: []       // subs when top-level cat was clicked
    property string parentCat: ""
    property string parentLabel: ""
    property var    categoryApps: []
    property bool   loading: false
    property bool   showAll: false       // true after "See all →" is clicked

    // Top-level view: limit to 12 apps; leaf view: show all
    readonly property int topLimit: 12
    property var displayedApps: {
        if (subcatList.length > 0 && !showAll)
            return categoryApps.slice(0, topLimit);
        return categoryApps;
    }

    // ── Public API ─────────────────────────────────────────────────────────────

    function loadCategoryWithSubs(cat, label, subcats) {
        _reset(cat, label);
        subcatList = subcats || [];
        parentCat = "";
        parentLabel = "";
        showAll = false;
        _fetchApps(cat);
    }

    function showCategory(cat, label) {
        _reset(cat, label);
        subcatList = [];
        showAll = false;
        _fetchApps(cat);
    }

    function goBack() {
        if (parentCat !== "") {
            var tree = root.categoryTree;
            var subs = [];
            for (var i = 0; i < tree.length; i++) {
                if (tree[i].cat === parentCat) { subs = tree[i].subs; break; }
            }
            loadCategoryWithSubs(parentCat, parentLabel, subs);
        }
    }

    // ── Internal ───────────────────────────────────────────────────────────────

    function _reset(cat, label) {
        pollTimer.stop();
        selectedCategory = cat;
        selectedLabel = label;
        categoryApps = [];
        loading = false;
        showAll = false;
    }

    function _fetchApps(cat) {
        loading = true;
        categoryApps = [];
        backend.loadCategory(cat);
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
                    try { categoryApps = JSON.parse(backend.readLog()); }
                    catch(e) { categoryApps = []; }
                }
            }
        }
    }

    // ── UI ─────────────────────────────────────────────────────────────────────

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── Back bar (shown when in a subcategory) ─────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            height: 44
            color: palette.button
            visible: parentCat !== "" && !showAll

            RowLayout {
                anchors { fill: parent; leftMargin: 12; rightMargin: 12 }
                spacing: 8

                Button {
                    text: "← " + (parentLabel !== "" ? parentLabel : "Back")
                    flat: true
                    font.pixelSize: 12
                    onClicked: explorePage.goBack()
                }

                Label {
                    text: selectedLabel
                    font.pixelSize: 15
                    font.bold: true
                }

                Item { Layout.fillWidth: true }
            }
        }

        // ── "See all" back bar (shown when viewing all apps from top-level) ────
        Rectangle {
            Layout.fillWidth: true
            height: 44
            color: palette.button
            visible: showAll

            RowLayout {
                anchors { fill: parent; leftMargin: 12; rightMargin: 12 }
                spacing: 8

                Button {
                    text: "← " + selectedLabel
                    flat: true
                    font.pixelSize: 12
                    onClicked: { showAll = false; }
                }

                Label {
                    text: "All " + selectedLabel + " Apps"
                    font.pixelSize: 15
                    font.bold: true
                }

                Item { Layout.fillWidth: true }
            }
        }

        // ── Category heading (top-level, no back bar) ──────────────────────────
        Item {
            Layout.fillWidth: true
            height: 44
            visible: selectedLabel !== "" && parentCat === "" && !showAll

            Label {
                anchors { left: parent.left; leftMargin: 24; verticalCenter: parent.verticalCenter }
                text: selectedLabel
                font.pixelSize: 18
                font.bold: true
            }
        }

        // ── Scrollable content ─────────────────────────────────────────────────
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            Column {
                width: parent.width
                topPadding: 8
                bottomPadding: 24
                spacing: 0

                // ── Subcategory 2-column tile grid ─────────────────────────────
                Item {
                    width: parent.width
                    height: subcatGrid.implicitHeight + 24
                    visible: explorePage.subcatList.length > 0 && !showAll

                    Grid {
                        id: subcatGrid
                        anchors {
                            left: parent.left; right: parent.right
                            top: parent.top; margins: 16
                        }
                        columns: 2
                        columnSpacing: 10
                        rowSpacing: 10

                        Repeater {
                            model: explorePage.subcatList

                            Rectangle {
                                width: (subcatGrid.width - subcatGrid.columnSpacing) / 2
                                height: 68
                                radius: 8
                                color: tileArea.containsMouse ? palette.highlight : palette.button
                                border.color: palette.mid
                                border.width: 1

                                RowLayout {
                                    anchors { fill: parent; leftMargin: 14; rightMargin: 10 }
                                    spacing: 8

                                    Label {
                                        text: modelData.label
                                        font.pixelSize: 13
                                        font.bold: true
                                        Layout.fillWidth: true
                                        color: tileArea.containsMouse
                                               ? palette.highlightedText : palette.text
                                    }

                                    Label {
                                        text: "›"
                                        font.pixelSize: 20
                                        color: tileArea.containsMouse
                                               ? palette.highlightedText : palette.mid
                                    }
                                }

                                MouseArea {
                                    id: tileArea
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        explorePage.parentCat   = explorePage.selectedCategory;
                                        explorePage.parentLabel = explorePage.selectedLabel;
                                        explorePage.showCategory(modelData.cat, modelData.label);
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Top Apps header + "See all" button ─────────────────────────
                Item {
                    width: parent.width
                    height: 44
                    visible: subcatList.length > 0 && !showAll
                             && !loading && categoryApps.length > 0

                    RowLayout {
                        anchors { fill: parent; leftMargin: 16; rightMargin: 16 }

                        Label {
                            text: "Top " + selectedLabel + " Apps"
                            font.pixelSize: 14
                            font.bold: true
                            Layout.fillWidth: true
                        }

                        Button {
                            visible: categoryApps.length > topLimit
                            text: "See all " + categoryApps.length + " →"
                            flat: true
                            font.pixelSize: 12
                            onClicked: { showAll = true; }
                        }
                    }
                }

                // ── Loading indicator ──────────────────────────────────────────
                Item {
                    width: parent.width
                    height: 60
                    visible: loading

                    Row {
                        anchors.centerIn: parent
                        spacing: 12
                        BusyIndicator { running: loading; implicitWidth: 28; implicitHeight: 28 }
                        Label {
                            text: "Loading apps…"
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                }

                // ── Empty state ────────────────────────────────────────────────
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    topPadding: 40
                    text: "No apps found in this category."
                    color: palette.mid
                    font.pixelSize: 14
                    visible: !loading && categoryApps.length === 0 && selectedCategory !== ""
                }

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    topPadding: 60
                    text: "Select a category from the sidebar to browse apps."
                    color: palette.mid
                    font.pixelSize: 14
                    visible: !loading && selectedCategory === ""
                }

                // ── App grid (Flow — wraps to fill width) ──────────────────────
                Flow {
                    width: parent.width - 32
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 12
                    visible: !loading && displayedApps.length > 0
                    topPadding: 4

                    Repeater {
                        model: loading ? [] : explorePage.displayedApps

                        Rectangle {
                            width: 120
                            height: 120
                            radius: 10
                            color: appCardArea.containsMouse ? Qt.lighter(palette.button, 1.08) : palette.button
                            border.color: palette.mid
                            border.width: 1

                            Column {
                                anchors { fill: parent; margins: 8 }
                                spacing: 6

                                AppIcon {
                                    iconPath: modelData.icon_path || ""
                                    iconName: modelData.name || modelData.id || "?"
                                    size: 56
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

                            // Installed checkmark badge
                            Rectangle {
                                visible: modelData.installed === true
                                width: 16; height: 16
                                radius: 8
                                color: "#4caf50"
                                anchors { top: parent.top; right: parent.right; margins: 4 }
                                Label {
                                    anchors.centerIn: parent
                                    text: "✓"
                                    font.pixelSize: 9
                                    color: "white"
                                }
                            }

                            MouseArea {
                                id: appCardArea
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
