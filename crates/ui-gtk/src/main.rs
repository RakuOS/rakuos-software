// rakuos-software-gtk — GTK4/libadwaita software center frontend

mod pages;

use gtk4::prelude::*;
use gtk4::glib;
use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow};

const APP_ID: &str = "org.rakuos.Software";

fn main() -> glib::ExitCode {
    env_logger::init();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("RakuOS Software")
        .default_width(1000)
        .default_height(700)
        .build();

    let nav_view = pages::build_main_view();
    window.set_content(Some(&nav_view));
    window.present();
}
