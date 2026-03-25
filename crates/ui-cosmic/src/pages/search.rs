// ui-cosmic/pages/search.rs

use crate::Message;
use cosmic::{widget, Element};
use cosmic::iced::Length;

pub fn view<'a>() -> Element<'a, Message> {
    widget::container(
        widget::text("Search page — TODO")
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
