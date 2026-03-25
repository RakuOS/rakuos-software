import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "../components"

Item {
    id: explorePage

    // ── State ──────────────────────────────────────────────────────────────────
    property string selectedCategory: ""
    property string selectedLabel: ""
    property var    subcatList: []          // subs when top-level cat was clicked
    property string parentCat: ""
    property string parentLabel: ""
    property var    categoryApps: []
    property bool   loading: false

    // ── Public API (called from main.qml and sidebar) ─────────────────────────

    // Called when sidebar top-level cat is clicked — shows subcat tiles + top apps
    function loadCategoryWithSubs(cat, label, subcats) {
        _reset(cat, label);
        subcatList = subcats || [];
        parentCat = "";
        parentLabel = "";
        // Load apps for this top-level category (shown below subcat tiles)
        _fetchApps(cat);
    }

    // Called when subcat tile or subcat tree item clicked — shows app list
    function showCategory(cat, label) {
        _reset(cat, label);
        subcatList = [];
        // Keep parent info for back button (set by caller if needed)
        _fetchApps(cat);
    }

    // Back button → restore parent category view
    function goBack() {
        if (parentCat !== "") {
            // Find subcats for the parent from main's categoryTree
            var tree = root.categoryTree;
            var subs = [];
            for (var i = 0; i < tree.length; i++) {
                if (tree[i].cat === parentCat) { subs = tree[i].subs; break; }
            }
            loadCategoryWithSubs(parentCat, parentLabel, subs);
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    function _reset(cat, label) {
        pollTimer.stop();
        selectedCategory = cat;
        selectedLabel = label;
        categoryApps = [];
        loading = false;
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

    // ── UI ────────────────────────────────────────────────────────────────────

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── Back bar (shown when in a subcategory with a parent) ───────────────
        Rectangle {
            Layout.fillWidth: true
            height: 44
            color: palette.button
            visible: parentCat !== ""

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

        // ── Category heading ───────────────────────────────────────────────────
        Item {
            Layout.fillWidth: true
            height: selectedLabel !== "" ? 44 : 0
            visible: selectedLabel !== ""

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
                bottomPadding: 16
                spacing: 0

                // ── Subcategory tiles (2-column grid, Discover style) ──────────
                Item {
                    width: parent.width
                    height: subcatGrid.implicitHeight + 24
                    visible: explorePage.subcatList.length > 0

                    Grid {
                        id: subcatGrid
                        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 16 }
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
                                        color: tileArea.containsMouse ? palette.highlightedText : palette.text
                                    }

                                    Label {
                                        text: "›"
                                        font.pixelSize: 20
                                        color: tileArea.containsMouse ? palette.highlightedText : palette.mid
                                    }
                                }

                                MouseArea {
                                    id: tileArea
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        explorePage.parentCat = explorePage.selectedCategory;
                                        explorePage.parentLabel = explorePage.selectedLabel;
                                        explorePage.showCategory(modelData.cat, modelData.label);
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Top apps section header (shown below subcat tiles) ─────────
                Item {
                    width: parent.width
                    height: 40
                    visible: explorePage.subcatList.length > 0 && !loading && categoryApps.length > 0

                    RowLayout {
                        anchors { fill: parent; leftMargin: 16; rightMargin: 16 }

                        Label {
                            text: "Top " + selectedLabel + " Apps"
                            font.pixelSize: 14
                            font.bold: true
                            Layout.fillWidth: true
                        }
                    }
                }

                // ── Loading indicator ─────────────────────────────────────────
                Item {
                    width: parent.width
                    height: 60
                    visible: loading

                    Row {
                        anchors.centerIn: parent
                        spacing: 12
                        BusyIndicator { running: loading; implicitWidth: 28; implicitHeight: 28 }
                        Label { text: "Loading apps…"; anchors.verticalCenter: parent.verticalCenter }
                    }
                }

                // ── No apps state ─────────────────────────────────────────────
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    topPadding: 40
                    text: "No apps found in this category."
                    color: palette.mid
                    font.pixelSize: 14
                    visible: !loading && categoryApps.length === 0 && selectedCategory !== ""
                }

                // ── Empty state (no category selected) ───────────────────────
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    topPadding: 60
                    text: "Select a category from the sidebar to browse apps."
                    color: palette.mid
                    font.pixelSize: 14
                    visible: !loading && selectedCategory === ""
                }

                // ── App list ──────────────────────────────────────────────────
                Repeater {
                    model: loading ? [] : categoryApps

                    Rectangle {
                        width: explorePage.width
                        height: 56
                        color: rowArea.containsMouse ? palette.highlight : "transparent"

                        RowLayout {
                            anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                            spacing: 12

                            AppIcon {
                                iconPath: modelData.icon_path || ""
                                iconName: modelData.name || modelData.id || "?"
                                size: 36
                            }

                            Column {
                                Layout.fillWidth: true
                                spacing: 2

                                Label {
                                    text: modelData.name || modelData.id || ""
                                    font.bold: true
                                    elide: Text.ElideRight
                                    width: parent.width
                                    color: rowArea.containsMouse ? palette.highlightedText : palette.text
                                }

                                Label {
                                    text: modelData.summary || ""
                                    font.pixelSize: 11
                                    color: rowArea.containsMouse ? palette.highlightedText : palette.mid
                                    elide: Text.ElideRight
                                    width: parent.width
                                    visible: text !== ""
                                }
                            }

                            // Source badge
                            Rectangle {
                                height: 18
                                width: srcLbl.implicitWidth + 10
                                radius: 3
                                color: {
                                    var s = modelData.source || "";
                                    if (s === "flatpak") return "#e8e0f0";
                                    if (s === "webapp")  return "#e3f2fd";
                                    return palette.button;
                                }

                                Label {
                                    id: srcLbl
                                    anchors.centerIn: parent
                                    text: modelData.source || ""
                                    font.pixelSize: 9
                                    color: "#555"
                                }
                            }

                            Label {
                                text: "✓"
                                color: "#4caf50"
                                font.pixelSize: 14
                                visible: modelData.installed === true
                            }

                            Button {
                                text: "Install"
                                visible: modelData.installed !== true
                                flat: true
                                onClicked: backend.installApp(modelData.id || "", modelData.source || "")
                            }
                        }

                        Rectangle {
                            anchors { bottom: parent.bottom; left: parent.left; right: parent.right; leftMargin: 16; rightMargin: 16 }
                            height: 1
                            color: palette.mid
                            opacity: 0.15
                        }

                        MouseArea {
                            id: rowArea
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
