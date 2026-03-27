#include <QCoreApplication>
#include <QGuiApplication>
#include <QIcon>

extern "C" void set_qt_app_properties() {
    // Ensure Qt can find image format plugins (WebP, AVIF, etc.)
    QCoreApplication::addLibraryPath("/usr/lib64/qt6/plugins");
    QCoreApplication::addLibraryPath("/usr/lib/qt6/plugins");

    QGuiApplication::setDesktopFileName("org.rakuos.Software");
    QGuiApplication::setWindowIcon(QIcon("/usr/share/pixmaps/rakuos-logo.png"));
}
