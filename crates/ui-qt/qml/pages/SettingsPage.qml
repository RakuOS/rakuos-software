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

            TabButton { text: "Updates" }
            TabButton { text: "Flatpak Repositories" }
            TabButton { text: "Firmware / LVFS" }
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
                        color: palette.mid
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

                    CheckBox {
                        id: checkPkgs
                        text: "Overlay package updates"
                        checked: true
                    }

                    CheckBox {
                        id: checkFlatpak
                        text: "Flatpak updates"
                        checked: true
                    }

                    CheckBox {
                        id: checkImage
                        text: "System image updates (bootc)"
                        checked: true
                    }

                    CheckBox {
                        id: checkAI
                        text: "AppImage updates"
                        checked: true
                    }

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
                        color: palette.mid
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
            ScrollView {
                contentWidth: availableWidth
                clip: true

                Column {
                    width: parent.width
                    topPadding: 20
                    leftPadding: 24
                    rightPadding: 24
                    bottomPadding: 20
                    spacing: 12

                    Label {
                        text: "Flatpak Repositories"
                        font.pixelSize: 16
                        font.bold: true
                    }

                    Label {
                        text: "Manage Flatpak repositories. Use the command line to add or remove remotes:\nflatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo"
                        color: palette.mid
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        width: parent.width
                    }

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: palette.mid
                        opacity: 0.3
                    }

                    Button {
                        text: "Add Flathub (System)"
                        onClicked: {
                            // Would run flatpak remote-add
                        }
                    }

                    Label {
                        text: "Full Flatpak repository management will be available in a future update."
                        color: palette.mid
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        width: parent.width
                    }
                }
            }

            // ── Firmware tab ─────────────────────────────────────────────────
            ScrollView {
                contentWidth: availableWidth
                clip: true

                Column {
                    width: parent.width
                    topPadding: 20
                    leftPadding: 24
                    rightPadding: 24
                    bottomPadding: 20
                    spacing: 12

                    Label {
                        text: "Firmware & LVFS"
                        font.pixelSize: 16
                        font.bold: true
                    }

                    Label {
                        text: "Manage firmware update sources via fwupd. LVFS provides updates from hardware vendors."
                        color: palette.mid
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        width: parent.width
                    }

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: palette.mid
                        opacity: 0.3
                    }

                    Button {
                        text: "Refresh Firmware Metadata"
                        onClicked: {
                            // Would run fwupdmgr refresh
                        }
                    }

                    Button {
                        text: "Check for Firmware Updates"
                        onClicked: {
                            // Would run fwupdmgr get-updates
                        }
                    }

                    Label {
                        text: "Full firmware management will be available in a future update."
                        color: palette.mid
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        width: parent.width
                    }
                }
            }
        }
    }

    Component.onCompleted: loadSettings()
}
