import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import org.rakuos.software 1.0
import "pages"
import "components"

ApplicationWindow {
    id: root
    visible: true
    width: 1280
    height: 800
    minimumWidth: 900
    minimumHeight: 600
    title: "RakuOS Software"

    // Close to tray — hide window instead of quitting
    onClosing: function(close) {
        close.accepted = false;
        root.visible = false;
    }

    SoftwareBackend { id: backend }

    // ── Cache readiness ───────────────────────────────────────────────────────
    property bool cacheReady: false

    Timer {
        id: cacheCheckTimer
        interval: 200
        repeat: true
        running: !root.cacheReady
        onTriggered: {
            if (backend.isCacheReady()) {
                root.cacheReady = true;
            }
        }
    }

    // ── Local file install ────────────────────────────────────────────────────
    function showLocalFile(path) {
        var kind = backend.fileKind(path);
        if (kind === "unknown") return;
        localFilePage.activate(path, kind);
        backend.navigate(10);
    }

    // ── Startup init ──────────────────────────────────────────────────────────
    Component.onCompleted: {
        // Pre-warm AppStream cache so first search/browse is instant
        backend.warmCache();
        // Load any cached update count from the last daemon check for badge display
        backend.loadDaemonUpdateCache();
        // If launched via MIME type with a file argument, open it
        var startupPath = backend.readStartupFilePath();
        if (startupPath !== "") {
            Qt.callLater(function() { root.showLocalFile(startupPath); });
        }
    }

    // ── Show-request / quit-request poll (tray interactions) ─────────────────
    Timer {
        id: showRequestTimer
        interval: 500
        repeat: true
        running: true
        onTriggered: {
            if (backend.checkQuitRequest()) {
                Qt.quit();
                return;
            }
            if (backend.checkShowRequest()) {
                root.visible = true;
                root.raise();
                root.requestActivate();
                backend.navigate(0);   // always return to Home
            }
        }
    }

    // Dim secondary text that works in both light and dark mode
    property color dimText: Qt.rgba(palette.windowText.r, palette.windowText.g, palette.windowText.b, 0.6)

    // Human-readable source label — native and terra are both "RakuOS Linux"
    function sourceLabel(src) {
        if (src === "flatpak") return "Flatpak";
        if (src === "webapp")  return "Web App";
        return "RakuOS Linux";
    }

    // Badge color for a source
    function sourceColor(src) {
        if (src === "flatpak") return "#1565c0";
        if (src === "webapp")  return "#00695c";
        return "#37474f";   // native, terra — same color
    }

    // Global detail page navigation
    property var detailApp: null
    property int previousPage: 0

    function showDetail(appData) {
        detailApp = appData;
        previousPage = backend.currentPage;
        backend.navigate(7);  // page 7 = detail
    }

    function hideDetail() {
        backend.navigate(previousPage);
    }

    // Navigate to explore page with a category (called from sidebar tree)
    function navigateCategory(cat, label, subcats) {
        backend.navigate(1);
        if (subcats && subcats.length > 0) {
            explorePage.loadCategoryWithSubs(cat, label, subcats);
        } else {
            explorePage.showCategory(cat, label);
        }
    }

    // ── Category tree data (matches Python CATEGORY_TREE exactly) ─────────────
    property var categoryTree: [
        { label: "🎮 Games",        cat: "Game",        subs: [
            { label: "Action",         cat: "ActionGame"    },
            { label: "Adventure",      cat: "AdventureGame" },
            { label: "Arcade",         cat: "ArcadeGame"    },
            { label: "Board Games",    cat: "BoardGame"     },
            { label: "Card Games",     cat: "CardGame"      },
            { label: "Emulators",      cat: "Emulator"      },
            { label: "Kids",           cat: "KidsGame"      },
            { label: "Logic",          cat: "LogicGame"     },
            { label: "Role Playing",   cat: "RolePlaying"   },
            { label: "Shooter",        cat: "Shooter"       },
            { label: "Simulation",     cat: "Simulation"    },
            { label: "Sports",         cat: "SportsGame"    },
            { label: "Strategy",       cat: "StrategyGame"  },
        ]},
        { label: "🌐 Internet",      cat: "Network",     subs: [
            { label: "Web Browsers",   cat: "WebBrowser"      },
            { label: "Email",          cat: "Email"           },
            { label: "Chat & IM",      cat: "InstantMessaging" },
            { label: "File Sharing",   cat: "FileTransfer"    },
            { label: "News & RSS",     cat: "News"            },
            { label: "Remote Access",  cat: "RemoteAccess"    },
        ]},
        { label: "🎵 Audio & Video", cat: "AudioVideo",  subs: [
            { label: "Music",          cat: "Music"           },
            { label: "Video",          cat: "Video"           },
            { label: "Editors",        cat: "AudioVideoEditing" },
            { label: "MIDI",           cat: "Midi"            },
            { label: "TV",             cat: "TV"              },
            { label: "Recorders",      cat: "Recorder"        },
            { label: "Mixers",         cat: "Mixer"           },
        ]},
        { label: "📷 Graphics",      cat: "Graphics",    subs: [
            { label: "2D Editors",     cat: "2DGraphics"      },
            { label: "3D Editors",     cat: "3DGraphics"      },
            { label: "Photography",    cat: "Photography"     },
            { label: "Viewers",        cat: "Viewer"          },
            { label: "Publishing",     cat: "Publishing"      },
            { label: "Scanning",       cat: "Scanning"        },
        ]},
        { label: "💼 Productivity",  cat: "Office",      subs: [
            { label: "Office Suite",   cat: "WordProcessor"   },
            { label: "Spreadsheets",   cat: "Spreadsheet"     },
            { label: "Presentation",   cat: "Presentation"    },
            { label: "Calendar",       cat: "Calendar"        },
            { label: "Finance",        cat: "Finance"         },
            { label: "Contacts",       cat: "ContactManagement" },
            { label: "Project Mgmt",   cat: "ProjectManagement" },
        ]},
        { label: "🛠 Development",   cat: "Development", subs: [
            { label: "IDEs",           cat: "IDE"             },
            { label: "Version Control",cat: "RevisionControl" },
            { label: "Debuggers",      cat: "Debugger"        },
            { label: "Web Dev",        cat: "WebDevelopment"  },
            { label: "Database",       cat: "Database"        },
            { label: "Data Viz",       cat: "DataVisualization" },
        ]},
        { label: "⚙️ System",         cat: "System",      subs: [
            { label: "File Managers",  cat: "FileManager"     },
            { label: "Terminal",       cat: "TerminalEmulator" },
            { label: "Monitors",       cat: "Monitor"         },
            { label: "Security",       cat: "Security"        },
            { label: "Settings",       cat: "Settings"        },
            { label: "Package Mgmt",   cat: "PackageManager"  },
        ]},
        { label: "🧰 Utilities",     cat: "Utility",     subs: [
            { label: "Archiving",      cat: "Archiving"       },
            { label: "Calculators",    cat: "Calculator"      },
            { label: "Clocks",         cat: "Clock"           },
            { label: "Text Editors",   cat: "TextEditor"      },
            { label: "File Tools",     cat: "FileTools"       },
            { label: "Dictionary",     cat: "Dictionary"      },
        ]},
        { label: "🔬 Science",       cat: "Science",     subs: [
            { label: "Astronomy",      cat: "Astronomy"       },
            { label: "Chemistry",      cat: "Chemistry"       },
            { label: "Electronics",    cat: "Electronics"     },
            { label: "Engineering",    cat: "Engineering"     },
            { label: "Geography",      cat: "Geography"       },
            { label: "Physics",        cat: "Physics"         },
        ]},
        { label: "🎓 Education",     cat: "Education",   subs: [
            { label: "Languages",      cat: "Languages"       },
            { label: "Mathematics",    cat: "Math"            },
            { label: "Science",        cat: "Science"         },
            { label: "Literature",     cat: "Literature"      },
            { label: "Geography",      cat: "Geography"       },
            { label: "Kids",           cat: "KidsGame"        },
        ]},
    ]

    // Track which top-level category sections are expanded
    property var expandedCats: ({})

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // ── Sidebar ────────────────────────────────────────────────────────────
        Rectangle {
            Layout.preferredWidth: 230
            Layout.fillHeight: true
            color: palette.base
            visible: backend.currentPage !== 7

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                // ── Logo row ────────────────────────────────────────────────────
                Item {
                    Layout.fillWidth: true
                    height: 54

                    RowLayout {
                        anchors { fill: parent; leftMargin: 14; rightMargin: 8 }
                        spacing: 8

                        Image {
                            source: "file:///usr/share/pixmaps/rakuos-logo.png"
                            sourceSize.width: 28
                            sourceSize.height: 28
                            Layout.preferredWidth: 28
                            Layout.preferredHeight: 28
                            fillMode: Image.PreserveAspectFit
                            smooth: true
                            mipmap: true
                        }

                        Label {
                            text: "RakuOS Software"
                            font.pixelSize: 13
                            font.bold: true
                            Layout.fillWidth: true
                        }
                    }
                }

                Rectangle { Layout.fillWidth: true; height: 1; color: palette.mid; opacity: 0.3 }

                // ── Nav buttons + category tree ─────────────────────────────────
                ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    contentWidth: availableWidth
                    clip: true
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                    Column {
                        width: parent.width
                        topPadding: 8
                        bottomPadding: 12

                        // ── Static nav buttons ──────────────────────────────────
                        Repeater {
                            model: [
                                { label: "🏠  Home",       page: 0 },
                                { label: "📦  Installed",  page: 3 },
                                { label: "🌐  Web Apps",   page: 8 },
                                { label: "🔄  Updates",    page: 4 },
                                { label: "⚙️   System",    page: 5 },
                                { label: "🛠  Settings",   page: 6 },
                            ]
                            delegate: Rectangle {
                                width: parent.width
                                height: 34
                                color: backend.currentPage === modelData.page
                                    ? Qt.rgba(palette.highlight.r, palette.highlight.g, palette.highlight.b, 0.15)
                                    : navBtnArea.containsMouse ? palette.button : "transparent"
                                radius: 4

                                Label {
                                    anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
                                    text: modelData.label
                                    font.pixelSize: 13
                                    font.bold: backend.currentPage === modelData.page
                                    color: backend.currentPage === modelData.page
                                        ? palette.highlight : palette.text
                                }

                                MouseArea {
                                    id: navBtnArea
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: backend.navigate(modelData.page)
                                }
                            }
                        }

                        // ── Separator + Categories heading ──────────────────────
                        Item { width: parent.width; height: 10 }

                        Rectangle { width: parent.width - 20; height: 1; color: palette.mid; opacity: 0.3; anchors.horizontalCenter: parent.horizontalCenter }

                        Item { width: parent.width; height: 6 }

                        Label {
                            leftPadding: 14
                            text: "Categories"
                            font.pixelSize: 11
                            font.bold: true
                            color: root.dimText
                        }

                        Item { width: parent.width; height: 4 }

                        // ── Expandable category tree ────────────────────────────
                        Repeater {
                            model: root.categoryTree

                            Column {
                                id: catSection
                                width: parent.width

                                property bool expanded: root.expandedCats[modelData.cat] === true

                                // Top-level row: arrow toggle + category button
                                Rectangle {
                                    width: parent.width
                                    height: 32
                                    color: topCatArea.containsMouse ? palette.button : "transparent"
                                    radius: 4

                                    RowLayout {
                                        anchors { fill: parent; leftMargin: 4; rightMargin: 6 }
                                        spacing: 0

                                        // Arrow toggle
                                        Label {
                                            text: catSection.expanded ? "▾" : "▸"
                                            font.pixelSize: 10
                                            color: root.dimText
                                            Layout.preferredWidth: 18
                                            horizontalAlignment: Text.AlignHCenter
                                        }

                                        Label {
                                            text: modelData.label
                                            font.pixelSize: 12
                                            color: backend.currentPage === 1 ? palette.text : palette.text
                                            Layout.fillWidth: true
                                        }
                                    }

                                    MouseArea {
                                        id: topCatArea
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            // Toggle expand
                                            var e = root.expandedCats;
                                            e[modelData.cat] = !catSection.expanded;
                                            root.expandedCats = e;
                                            catSection.expanded = e[modelData.cat];
                                            // Navigate to explore with subcats
                                            root.navigateCategory(modelData.cat, modelData.label, modelData.subs);
                                        }
                                    }
                                }

                                // Sub-items (shown when expanded)
                                Column {
                                    width: parent.width
                                    visible: catSection.expanded

                                    Repeater {
                                        model: modelData.subs

                                        Rectangle {
                                            width: parent.width
                                            height: 27
                                            color: subArea.containsMouse ? palette.button : "transparent"
                                            radius: 4

                                            Label {
                                                anchors { left: parent.left; leftMargin: 30; verticalCenter: parent.verticalCenter }
                                                text: modelData.label
                                                font.pixelSize: 11
                                                color: palette.text
                                            }

                                            MouseArea {
                                                id: subArea
                                                anchors.fill: parent
                                                hoverEnabled: true
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: {
                                                    // Navigate to explore with this subcategory
                                                    backend.navigate(1);
                                                    explorePage.showCategory(modelData.cat, modelData.label);
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
        }

        Rectangle {
            width: 1
            Layout.fillHeight: true
            color: palette.mid
            opacity: 0.35
            visible: backend.currentPage !== 7
        }

        // ── Main area ──────────────────────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // ── Topbar (search) ────────────────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                height: 56
                color: palette.button
                visible: backend.currentPage !== 7

                RowLayout {
                    anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                    spacing: 12

                    // Search field
                    Rectangle {
                        Layout.preferredWidth: 420
                        height: 34
                        radius: 6
                        color: palette.base
                        border.color: searchField.activeFocus ? palette.highlight : palette.mid
                        border.width: searchField.activeFocus ? 2 : 1

                        RowLayout {
                            anchors { fill: parent; leftMargin: 10; rightMargin: 6 }
                            spacing: 6

                            Label {
                                text: "🔍"
                                font.pixelSize: 14
                                color: root.dimText
                            }

                            TextField {
                                id: searchField
                                Layout.fillWidth: true
                                placeholderText: "Search apps…"
                                font.pixelSize: 13
                                background: Rectangle { color: "transparent" }
                                Keys.onReturnPressed: {
                                    var q = text.trim();
                                    if (q.length < 2) return;
                                    backend.navigate(2);
                                    searchPageItem.triggerSearch(q);
                                }
                            }
                        }
                    }

                    Item { Layout.fillWidth: true }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                height: 1
                color: palette.mid
                opacity: 0.3
                visible: backend.currentPage !== 7
            }

            // ── Page stack ─────────────────────────────────────────────────────
            StackLayout {
                id: pageStack
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: backend.currentPage

                HomePage      { id: homePage }
                ExplorePage   { id: explorePage }
                SearchPage    { id: searchPageItem }
                InstalledPage { id: installedPage }
                UpdatesPage   { id: updatesPage }
                SystemPage    { id: systemPageItem }
                SettingsPage  { id: settingsPage }

                // Page 7: Detail
                DetailPage {
                    id: detailPageItem
                    app: root.detailApp
                    onBackRequested: root.hideDetail()
                }

                // Page 8: Web Apps catalog
                WebAppsPage { id: webAppsPage }

                // Page 9: (reserved — AppImages removed, managed via Installed page)
                Item {}

                // Page 10: Local file install
                LocalFilePage {
                    id: localFilePage
                    onBackRequested: backend.navigate(0)
                }
            }
        }

        // Re-activate data-driven pages when navigated to
        Connections {
            target: backend
            function onCurrentPageChanged() {
                switch (backend.currentPage) {
                    case 3: installedPage.activate(); break;
                    case 4: updatesPage.activate(); break;
                    case 5: systemPageItem.activate(); break;
                    case 7: detailPageItem.loadApp(root.detailApp); break;
                    case 8: webAppsPage.activate(); break;
                }
            }
        }
    }

    // ── Startup loading overlay (direct ApplicationWindow child — no layout conflict) ──
    Rectangle {
        anchors.fill: parent
        color: palette.window
        visible: !root.cacheReady
        z: 100

        Column {
            anchors.centerIn: parent
            spacing: 20

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                source: "file:///usr/share/pixmaps/rakuos-logo.svg"
                width: 80; height: 80
                fillMode: Image.PreserveAspectFit
                smooth: true
                mipmap: true
            }

            BusyIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                running: true
                implicitWidth: 40; implicitHeight: 40
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Loading software catalog…"
                font.pixelSize: 14
                color: root.dimText
            }
        }
    }
}
