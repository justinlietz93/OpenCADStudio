//! Shared colour selector: a dropdown-style button that opens a list of named
//! colours (each shown with its swatch) plus the full ACI palette. Used by the
//! properties panel and every style editor so colour selection looks and
//! behaves the same everywhere.

use crate::app::Message;
use crate::ui::properties::acad_color_display;
use acadrust::types::Color as AcadColor;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

/// Which "logical" entries the colour list offers besides the standard ACI
/// colours.
#[derive(Clone, Copy, Default)]
pub struct ColorExtras {
    pub by_layer: bool,
    pub by_block: bool,
}

const PICKER_BG: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.12,
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

/// Encode a colour as the ACI integer string the style editors store
/// (ByBlock=0, ByLayer=256, indexed 1-255; RGB has no ACI slot → ByLayer).
pub fn color_to_aci_string(c: AcadColor) -> String {
    match c {
        AcadColor::ByBlock => "0".to_string(),
        AcadColor::ByLayer => "256".to_string(),
        AcadColor::Index(i) => i.to_string(),
        AcadColor::Rgb { .. } => "256".to_string(),
    }
}

/// Decode an ACI integer string back into an `AcadColor`.
pub fn aci_string_to_color(s: &str) -> AcadColor {
    match s.trim().parse::<i16>().unwrap_or(256) {
        0 => AcadColor::ByBlock,
        256 => AcadColor::ByLayer,
        n if (1..=255).contains(&n) => AcadColor::Index(n as u8),
        _ => AcadColor::ByLayer,
    }
}

/// A small colour square.
fn swatch<'a>(bg: Color) -> Element<'a, Message> {
    container(text("").width(13).height(13))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                },
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        })
        .width(13)
        .height(13)
        .into()
}

/// Build a colour selector.
///
/// * `current` — the currently selected colour (shown on the button).
/// * `open` — whether the colour list / palette is expanded.
/// * `extras` — whether ByLayer / ByBlock appear in the list.
/// * `on_select` — called with the chosen colour.
/// * `on_toggle` — opens / closes the list.
pub fn color_selector<'a>(
    current: AcadColor,
    open: bool,
    extras: ColorExtras,
    on_select: impl Fn(AcadColor) -> Message + 'a,
    on_toggle: Message,
) -> Element<'a, Message> {
    let (cur_bg, cur_name) = acad_color_display(current);

    // Closed button: current swatch + name + caret.
    let head = button(
        row![
            swatch(cur_bg),
            text(cur_name).size(11).color(TEXT),
            text(if open { " ▲" } else { " ▾" }).size(9).color(TEXT),
        ]
        .spacing(5)
        .align_y(iced::Center),
    )
    .on_press(on_toggle)
    .padding([3, 6])
    .width(170);

    if !open {
        return head.into();
    }

    // One named-colour row (swatch + name), selectable.
    let named_row = |color: AcadColor| -> Element<'a, Message> {
        let (bg, name) = acad_color_display(color);
        button(
            row![swatch(bg), text(name).size(11).color(TEXT)]
                .spacing(5)
                .align_y(iced::Center),
        )
        .on_press(on_select(color))
        .style(|_: &Theme, status| button::Style {
            background: matches!(status, button::Status::Hovered)
                .then_some(Background::Color(Color {
                    r: 0.25,
                    g: 0.25,
                    b: 0.30,
                    a: 1.0,
                })),
            ..Default::default()
        })
        .padding([2, 4])
        .width(Length::Fill)
        .into()
    };

    let mut list = column![].spacing(1);
    if extras.by_layer {
        list = list.push(named_row(AcadColor::ByLayer));
    }
    if extras.by_block {
        list = list.push(named_row(AcadColor::ByBlock));
    }
    for i in 1u8..=9 {
        list = list.push(named_row(AcadColor::Index(i)));
    }

    // Full ACI palette grid (10-255) below the named colours.
    const COLS: u16 = 16;
    let mut grid = column![].spacing(1);
    let mut idx: u16 = 1;
    while idx <= 255 {
        let mut r = row![].spacing(1);
        for _ in 0..COLS {
            if idx > 255 {
                break;
            }
            let ci = idx as u8;
            let (bg, _) = acad_color_display(AcadColor::Index(ci));
            r = r.push(
                button(text("").width(12).height(12))
                    .on_press(on_select(AcadColor::Index(ci)))
                    .style(move |_: &Theme, status| button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            color: if matches!(status, button::Status::Hovered) {
                                Color::WHITE
                            } else {
                                Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 0.4,
                                }
                            },
                            width: if matches!(status, button::Status::Hovered) {
                                1.5
                            } else {
                                1.0
                            },
                            radius: 1.0.into(),
                        },
                        ..Default::default()
                    })
                    .padding(0),
            );
            idx += 1;
        }
        grid = grid.push(r);
    }

    let popup = container(
        column![list, scrollable(grid).height(120)].spacing(4),
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(PICKER_BG)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 2.0.into(),
        },
        ..Default::default()
    })
    .padding(5)
    .width(220);

    column![head, popup].spacing(2).into()
}
