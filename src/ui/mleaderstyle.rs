//! Multileader Style Manager window — fills the entire OS window.

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

/// View-model borrowed from the app for one render of the dialog.
pub struct MLeaderStyleView<'a> {
    pub styles: Vec<String>,
    pub selected: &'a str,
    pub style: Option<&'a acadrust::objects::MultiLeaderStyle>,
    pub current: String,
    pub landing_distance: &'a str,
    pub landing_gap: &'a str,
    pub arrowhead_size: &'a str,
    pub text_height: &'a str,
    pub scale_factor: &'a str,
    pub break_gap: &'a str,
    pub first_seg_angle: &'a str,
    pub second_seg_angle: &'a str,
    pub max_points: &'a str,
    pub default_text: &'a str,
    pub line_color: &'a str,
    pub text_color: &'a str,
}

fn section<'a>(label: &'static str) -> Element<'a, Message> {
    text(label).size(11).color(ACCENT).into()
}

fn num_row<'a>(
    label: &'static str,
    placeholder: &'static str,
    value: &'a str,
    field: &'static str,
) -> Element<'a, Message> {
    row![
        text(label).size(11).color(DIM).width(150),
        text_input(placeholder, value)
            .on_input(move |v| Message::MLeaderStyleEdit { field, value: v })
            .size(11)
            .width(110),
    ]
    .spacing(8)
    .align_y(iced::Center)
    .into()
}

fn enum_row<'a>(
    label: &'static str,
    options: Vec<String>,
    selected: String,
    field: &'static str,
) -> Element<'a, Message> {
    row![
        text(label).size(11).color(DIM).width(150),
        pick_list(options, Some(selected), move |value| {
            Message::MLeaderStyleSetEnum { field, value }
        })
        .text_size(11)
        .width(190),
    ]
    .spacing(8)
    .align_y(iced::Center)
    .into()
}

fn chk<'a>(label: &'static str, val: bool, field: &'static str) -> Element<'a, Message> {
    checkbox(val)
        .label(label)
        .on_toggle(move |_| Message::MLeaderStyleToggle(field))
        .size(14)
        .text_size(11)
        .into()
}

pub fn view_window<'a>(v: MLeaderStyleView<'a>) -> Element<'a, Message> {
    // ── Toolbar ───────────────────────────────────────────────────────────
    let toolbar = container(
        row![
            button(text("New").size(11))
                .on_press(Message::MLeaderStyleDialogNew)
                .style(btn_s(true))
                .padding([4, 10]),
            button(text("Delete").size(11))
                .on_press(Message::MLeaderStyleDialogDelete)
                .style(btn_s(false))
                .padding([4, 10]),
            button(text("Set Current").size(11))
                .on_press(Message::MLeaderStyleDialogSetCurrent)
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
    let style_items: Vec<Element<'_, Message>> = v
        .styles
        .iter()
        .map(|name| {
            let is_sel = name.as_str() == v.selected;
            button(text(name.clone()).size(11))
                .on_press(Message::MLeaderStyleDialogSelect(name.clone()))
                .style(list_item(is_sel))
                .padding([4, 8])
                .width(Fill)
                .into()
        })
        .collect();

    let style_list = container(
        column![
            text(format!("Current: {}", v.current)).size(10).color(DIM),
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
    let details: Element<'_, Message> = if let Some(s) = v.style {
        scrollable(
            column![
                row![
                    text("Name:").size(11).color(DIM).width(150),
                    text(s.name.clone()).size(11),
                ]
                .spacing(8),
                // Leader Format
                section("Leader Format"),
                enum_row(
                    "Path type:",
                    ["Invisible", "StraightLineSegments", "Spline"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    format!("{:?}", s.path_type),
                    "path_type"
                ),
                num_row("Line color (ACI):", "256", v.line_color, "line_color"),
                num_row("Arrowhead size:", "0.18", v.arrowhead_size, "arrowhead_size"),
                num_row("Break gap size:", "0.125", v.break_gap, "break_gap"),
                // Leader Structure
                section("Leader Structure"),
                chk("Enable landing", s.enable_landing, "enable_landing"),
                chk("Enable dogleg", s.enable_dogleg, "enable_dogleg"),
                num_row("Landing distance:", "8.0", v.landing_distance, "landing_distance"),
                num_row("Landing gap:", "0.09", v.landing_gap, "landing_gap"),
                num_row("Max leader points:", "2", v.max_points, "max_points"),
                num_row("First seg. angle:", "0", v.first_seg_angle, "first_seg_angle"),
                num_row("Second seg. angle:", "0", v.second_seg_angle, "second_seg_angle"),
                num_row("Scale factor:", "1.0", v.scale_factor, "scale_factor"),
                chk("Annotative", s.is_annotative, "annotative"),
                // Content
                section("Content"),
                enum_row(
                    "Content type:",
                    ["None", "Block", "MText", "Tolerance"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    format!("{:?}", s.content_type),
                    "content_type"
                ),
                num_row("Default text:", "", v.default_text, "default_text"),
                num_row("Text height:", "0.18", v.text_height, "text_height"),
                num_row("Text color (ACI):", "256", v.text_color, "text_color"),
                enum_row(
                    "Text angle:",
                    ["ParallelToLastLeaderLine", "Horizontal", "Optimized"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    format!("{:?}", s.text_angle_type),
                    "text_angle_type"
                ),
                enum_row(
                    "Text alignment:",
                    ["Left", "Center", "Right"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    format!("{:?}", s.text_alignment),
                    "text_alignment"
                ),
                chk("Text frame", s.text_frame, "text_frame"),
                chk("Text always left", s.text_always_left, "text_always_left"),
                button(text("Apply").size(11))
                    .on_press(Message::MLeaderStyleApply)
                    .style(btn_s(true))
                    .padding([4, 14]),
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
