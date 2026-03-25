import QtQuick 2.15

// AppIcon — shows app icon from icon_path file, falls back to initials
Item {
    id: root

    property string iconPath: ""
    property string iconName: ""   // fallback initial letter
    property int size: 36

    width: size
    height: size

    // File-based icon
    Image {
        id: fileIcon
        anchors.fill: parent
        source: root.iconPath ? "file://" + root.iconPath : ""
        visible: root.iconPath !== "" && status === Image.Ready
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
    }

    // Initials fallback
    Rectangle {
        anchors.fill: parent
        radius: root.size * 0.18
        color: palette.button
        visible: !fileIcon.visible

        Text {
            anchors.centerIn: parent
            text: root.iconName ? root.iconName[0].toUpperCase() : "?"
            font.pixelSize: root.size * 0.44
            font.bold: true
            color: palette.buttonText
        }
    }
}
