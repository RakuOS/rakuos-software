// rakuos-software-cosmic — libcosmic software center frontend

mod pages;

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{self, Length, Subscription};
use cosmic::iced_core::layout::Limits;
use cosmic::{executor, widget, Application, ApplicationExt, Element};

const APP_ID: &str = "org.rakuos.Software";

pub struct SoftwareApp {
    core: Core,
    current_page: Page,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Home,
    Explore,
    Search,
    Installed,
    Updates,
    System,
    Settings,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
}

impl Application for SoftwareApp {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core { &self.core }
    fn core_mut(&mut self) -> &mut Core { &mut self.core }

    fn init(core: Core, (): ()) -> (Self, Task<Message>) {
        let mut app = Self {
            core,
            current_page: Page::Home,
        };
        app.set_header_title("RakuOS Software".into());
        (app, Task::none())
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => self.current_page = page,
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let nav_buttons = widget::row()
            .push(nav_btn("Home",      Page::Home,      &self.current_page))
            .push(nav_btn("Explore",   Page::Explore,   &self.current_page))
            .push(nav_btn("Search",    Page::Search,    &self.current_page))
            .push(nav_btn("Installed", Page::Installed, &self.current_page))
            .push(nav_btn("Updates",   Page::Updates,   &self.current_page))
            .push(nav_btn("System",    Page::System,    &self.current_page))
            .push(nav_btn("Settings",  Page::Settings,  &self.current_page))
            .spacing(4)
            .padding(iced::Padding::from([8, 16]));

        let page_content: Element<Message> = match self.current_page {
            Page::Home      => pages::home::view(),
            Page::Explore   => pages::explore::view(),
            Page::Search    => pages::search::view(),
            Page::Installed => pages::installed::view(),
            Page::Updates   => pages::updates::view(),
            Page::System    => pages::system::view(),
            Page::Settings  => pages::settings::view(),
        };

        widget::column()
            .push(nav_buttons)
            .push(widget::divider::horizontal::default())
            .push(
                widget::container(page_content)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .into()
    }
}

fn nav_btn<'a>(label: &'static str, page: Page, current: &Page) -> Element<'a, Message> {
    if &page == current {
        widget::button::suggested(label)
            .on_press(Message::Navigate(page))
            .into()
    } else {
        widget::button::standard(label)
            .on_press(Message::Navigate(page))
            .into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let settings = Settings::default()
        .size(iced::Size::new(1000.0, 700.0))
        .size_limits(Limits::NONE.min_width(800.0).min_height(600.0))
        .is_daemon(false);
    cosmic::app::run::<SoftwareApp>(settings, ())?;
    Ok(())
}
