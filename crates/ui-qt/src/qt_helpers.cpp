#include <QGuiApplication>
#include <QIcon>

extern "C" void set_qt_app_properties() {
    QGuiApplication::setDesktopFileName("org.rakuos.Software");
    QGuiApplication::setWindowIcon(QIcon("/usr/share/pixmaps/rakuos-logo.png"));
}
