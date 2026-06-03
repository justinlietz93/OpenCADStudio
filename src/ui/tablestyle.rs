//! Table Style Manager window — fills the entire OS window.

use crate::app::Message;
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Fill, Theme};

const TB: Color = Color {
    r: 0.13,
    g: 0.13,
    b: 0.13,
    a: 1.0,
};
const BG: Color = Color {
    r: 0.15,
    g: 0.15,
    b: 0.15,
    a: 1.0,
};
const BORDER: Color = Color {
    r: 0.35,
    g: 0.35,
    b: 0.35,
    a: 1.0,
};
const TEXT: Color = Color {
    r: 0.88,
    g: 0.88,
    b: 0.88,
    a: 1.0,
};
const DIM: Color = Color {
    r: 0.55,
    g: 0.55,
    b: 0.55,
    a: 1.0,
};
const ACCENT: Color = Color {
    r: 0.25,
    g: 0.50,
    b: 0.85,
    a: 1.0,
};
const ACTIVE: Color = Color {
    r: 0.20,
    g: 0.40,
    b: 0.70,
    a: 1.0,
};
const LIST: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.12,
    a: 1.0,
};

fn btn_s(accent: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, st| button::Style {
        background: Some(Background::Color(match (accent, st) {
            (true, button::Status::Hovered | button::Status::Pressed) => Color {
                r: 0.20,
                g: 0.42,
                b: 0.72,
                a: 1.0,
            },
            (false, button::Status::Hovered | button::Status::Pressed) => Color {
                r: 0.28,
                g: 0.28,
                b: 0.28,
                a: 1.0,
            },
            (true, _) => ACCENT,
            _ => Color {
                r: 0.22,
                g: 0.22,
                b: 0.22,
                a: 1.0,
            },
        })),
        text_color: TEXT,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn list_item(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, st| button::Style {
        background: Some(Background::Color(match (active, st) {
            (true, _) => ACTIVE,
            (false, button::Status::Hovered | button::Status::Pressed) => Color {
                r: 0.26,
                g: 0.26,
                b: 0.26,
                a: 1.0,
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: TEXT,
        ..Default::default()
    }
}

fn hdivider<'a>() -> Element<'a, Message> {
    container(Space::new().width(Fill).height(1))
        .width(Fill)
        .height(1)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BORDER)),
            ..Default::default()
        })
        .into()
}

fn vsep<'a>() -> Element<'a, Message> {
    container(Space::new().width(1).height(Fill))
        .width(1)
        .height(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BORDER)),
            ..Default::default()
        })
        .into()
}

pub fn view_window<'a>(
    styles: Vec<String>,
    selected: &'a str,
    selected_style: Option<&'a acadrust::objects::TableStyle>,
    hmargin_buf: &'a str,
    vmargin_buf: &'a str,
    cell_textstyle: &'a [String; 3],
    cell_height: &'a [String; 3],
    cell_textcolor: &'a [String; 3],
    cell_fillcolor: &'a [String; 3],
) -> Element<'a, Message> {
    // ── Toolbar ───────────────────────────────────────────────────────────
    let toolbar = container(
        row![
            button(text("New").size(11))
                .on_press(Message::TableStyleDialogNew)
                .style(btn_s(true))
                .padding([4, 10]),
            button(text("Delete").size(11))
                .on_press(Message::TableStyleDialogDelete)
                .style(btn_s(false))
                .padding([4, 10]),
        ]
        .spacing(4)
        .align_y(iced::Center),
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(TB)),
        ..Default::default()
    })
    .width(Fill)
    .padding([5, 8]);

    // ── Left: Style list ──────────────────────────────────────────────────
    let style_items: Vec<Element<'_, Message>> = styles
        .iter()
        .map(|name| {
            let is_sel = name.as_str() == selected;
            button(text(name.clone()).size(11))
                .on_press(Message::TableStyleDialogSelect(name.clone()))
                .style(list_item(is_sel))
                .padding([4, 8])
                .width(Fill)
                .into()
        })
        .collect();

    let style_list = container(
        column![
            text("Styles").size(10).color(DIM),
            container(scrollable(column(style_items).spacing(2)).height(Fill))
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(LIST)),
                    border: Border {
                        color: BORDER,
                        width: 1.0,
                        radius: 3.0.into()
                    },
                    ..Default::default()
                })
                .width(Fill)
                .height(Fill)
                .padding(2),
        ]
        .spacing(4)
        .height(Fill),
    )
    .width(200)
    .height(Fill)
    .padding(iced::Padding {
        top: 12.0,
        right: 8.0,
        bottom: 12.0,
        left: 12.0,
    });

    // ── Right: Details panel ──────────────────────────────────────────────
    let info_row = |label: &'static str, val: String| -> Element<'_, Message> {
        row![
            text(label).size(11).color(DIM).width(160),
            text(val).size(11),
        ]
        .spacing(8)
        .align_y(iced::Center)
        .into()
    };

    let cell_editor = |row_label: &'static str,
                       row: u8,
                       rs: &acadrust::objects::RowCellStyle|
     -> Element<'a, Message> {
        let r = row as usize;
        let cell_in = |label: &'static str,
                       placeholder: &'static str,
                       value: &'a str,
                       field: &'static str|
         -> Element<'a, Message> {
            row![
                text(label).size(11).color(DIM).width(150),
                text_input(placeholder, value)
                    .on_input(move |v| Message::TableStyleCellEdit { row, field, value: v })
                    .size(11)
                    .width(100),
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into()
        };
        column![
            text(row_label).size(11).color(ACCENT),
            cell_in("  Text style:", "Standard", &cell_textstyle[r], "textstyle"),
            cell_in("  Text height:", "0.18", &cell_height[r], "height"),
            cell_in("  Text color (ACI):", "256", &cell_textcolor[r], "textcolor"),
            cell_in("  Fill color (ACI):", "256", &cell_fillcolor[r], "fillcolor"),
            row![
                text("  Alignment:").size(11).color(DIM).width(150),
                pick_list(
                    [
                        "TopLeft",
                        "TopCenter",
                        "TopRight",
                        "MiddleLeft",
                        "MiddleCenter",
                        "MiddleRight",
                        "BottomLeft",
                        "BottomCenter",
                        "BottomRight",
                    ]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
                    Some(format!("{:?}", rs.alignment)),
                    move |value| Message::TableStyleCellSetAlign { row, value },
                )
                .text_size(11)
                .width(140),
            ]
            .spacing(8)
            .align_y(iced::Center),
            checkbox(rs.fill_enabled)
                .label("  Background fill enabled")
                .on_toggle(move |_| Message::TableStyleCellToggleFill(row))
                .size(14)
                .text_size(11),
            button(text("Apply cell").size(11))
                .on_press(Message::TableStyleCellApply(row))
                .style(btn_s(true))
                .padding([4, 12]),
        ]
        .spacing(3)
        .into()
    };

    let details: Element<'_, Message> = if let Some(s) = selected_style {
        scrollable(
            column![
                info_row("Name:", s.name.clone()),
                checkbox(s.annotative)
                    .label("Annotative")
                    .on_toggle(|_| Message::TableStyleToggleAnnotative)
                    .size(14)
                    .text_size(11),
                row![
                    text("H Margin:").size(11).color(DIM).width(160),
                    text_input("1.5", hmargin_buf)
                        .on_input(|v| Message::TableStyleEdit { field: "hmargin", value: v })
                        .size(11)
                        .width(100),
                ]
                .spacing(8)
                .align_y(iced::Center),
                row![
                    text("V Margin:").size(11).color(DIM).width(160),
                    text_input("1.5", vmargin_buf)
                        .on_input(|v| Message::TableStyleEdit { field: "vmargin", value: v })
                        .size(11)
                        .width(100),
                ]
                .spacing(8)
                .align_y(iced::Center),
                checkbox(s.title_suppressed)
                    .label("Title row suppressed")
                    .on_toggle(|_| Message::TableStyleToggle("title_sup"))
                    .size(14)
                    .text_size(11),
                checkbox(s.header_suppressed)
                    .label("Header row suppressed")
                    .on_toggle(|_| Message::TableStyleToggle("header_sup"))
                    .size(14)
                    .text_size(11),
                button(text("Apply margins").size(11))
                    .on_press(Message::TableStyleApply)
                    .style(btn_s(true))
                    .padding([4, 12]),
                cell_editor("Data Row:", 0, &s.data_row_style),
                cell_editor("Header Row:", 1, &s.header_row_style),
                cell_editor("Title Row:", 2, &s.title_row_style),
            ]
            .spacing(6)
            .padding([12, 12]),
        )
        .height(Fill)
        .into()
    } else {
        container(text("Select a style to view details.").size(11).color(DIM))
            .padding([12, 12])
            .into()
    };

    let right_panel = container(details).width(Fill).height(Fill);

    let body = row![style_list, vsep(), right_panel].height(Fill);

    container(column![toolbar, hdivider(), body].spacing(0))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG)),
            ..Default::default()
        })
        .width(Fill)
        .height(Fill)
        .into()
}
