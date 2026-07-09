//! Plot / Print dialog — a full plot setup surface rendered as an in-canvas
//! modal (Plan B). Bundles printer choice, paper, scale, offset, plot style,
//! quality and output options into one dialog; on commit it either sends the
//! current layout to a system printer (with the chosen options) or writes a
//! PDF. Styled to match the other OCS dialogs (dark pills + fields).

use crate::app::Message;
use crate::io::paper_sizes::PaperSize;
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Fill, Length, Theme};

const TB: Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 1.0 };
const BG: Color = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
const BORDER: Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
const TEXT: Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
const DIM: Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
const ACCENT: Color = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
const FIELD: Color = Color { r: 0.10, g: 0.10, b: 0.10, a: 1.0 };

/// Sentinel entries in the printer dropdown (not real printer names).
pub const OUT_DEFAULT: &str = "System default printer";
pub const OUT_PDF: &str = "Save to PDF file…";

/// One of the many boolean plot options (folded into a single message so the
/// dialog needn't carry a variant per checkbox).
#[derive(Debug, Clone, Copy)]
pub enum PlotFlag {
    Center,
    ScaleLw,
    Mono,
    Lineweights,
    WithStyles,
    Transparency,
    PaperspaceLast,
    HidePaperspace,
    Stamp,
    SaveLayout,
}

/// Every edit the Plot dialog can emit. Wrapped in `Message::PlotDlg` so the
/// top-level match stays a single arm.
#[derive(Debug, Clone)]
pub enum PlotDlgMsg {
    Close,
    Commit,
    Preview,
    Printer(String),
    Paper(String),
    Orientation(String),
    Rotation(String),
    Area(String),
    Scale(String),
    Quality(String),
    Shade(String),
    Copies(String),
    OffsetX(String),
    OffsetY(String),
    Dpi(String),
    Flag(PlotFlag),
    LoadStyle,
    ClearStyle,
    PickWindow,
}

/// Transient state backing the Plot dialog. Seeded from the layout's plot
/// settings when the dialog opens; consumed on commit.
#[derive(Debug, Clone)]
pub struct PlotDialogState {
    /// Printer names discovered on the system (via `lpstat`), never the
    /// sentinels.
    pub printers: Vec<String>,
    /// Chosen printer name, or `None` for the system default.
    pub printer: Option<String>,
    /// Output goes to a PDF file instead of a printer.
    pub to_file: bool,
    pub paper: String,
    pub orientation: String,
    pub rotation: String,
    pub copies: String,
    pub area: String,
    pub center: bool,
    pub offset_x: String,
    pub offset_y: String,
    pub scale: String,
    pub scale_lw: bool,
    pub quality: String,
    pub dpi: String,
    pub shade: String,
    pub mono: bool,
    pub lineweights: bool,
    pub with_styles: bool,
    pub transparency: bool,
    pub paperspace_last: bool,
    pub hide_paperspace: bool,
    pub stamp: bool,
    pub save_layout: bool,
    /// Display name of the active plot style table ("" = none).
    pub style_name: String,
}

impl Default for PlotDialogState {
    fn default() -> Self {
        Self {
            printers: Vec::new(),
            printer: None,
            to_file: false,
            paper: "A4".into(),
            orientation: "Landscape".into(),
            rotation: "0°".into(),
            copies: "1".into(),
            area: "Window".into(),
            center: true,
            offset_x: "0.0".into(),
            offset_y: "0.0".into(),
            scale: "Fit".into(),
            scale_lw: true,
            quality: "Normal".into(),
            dpi: "300".into(),
            shade: "As displayed".into(),
            mono: false,
            lineweights: true,
            with_styles: true,
            transparency: false,
            paperspace_last: false,
            hide_paperspace: false,
            stamp: false,
            save_layout: false,
            style_name: String::new(),
        }
    }
}

impl PlotDialogState {
    /// Load the persisted print preferences (printer, copies, quality, output
    /// options) from `<config>/OpenCADStudio/plot.txt`. Drawing-specific fields
    /// (paper, scale, offset, rotation…) are NOT persisted here — they are
    /// seeded from the active layout each time the dialog opens.
    pub fn load() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::default()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut s = Self::default();
            if let Some(body) = prefs_path().and_then(|p| std::fs::read_to_string(p).ok()) {
                let flag = |v: &str| v == "1";
                for line in body.lines() {
                    let Some((k, v)) = line.split_once('=') else {
                        continue;
                    };
                    let (k, v) = (k.trim(), v.trim());
                    match k {
                        "printer" => {
                            s.printer = if v.is_empty() { None } else { Some(v.to_string()) }
                        }
                        "to_file" => s.to_file = flag(v),
                        "area" => s.area = v.to_string(),
                        "scale" => s.scale = v.to_string(),
                        "copies" => s.copies = v.to_string(),
                        "quality" => s.quality = v.to_string(),
                        "dpi" => s.dpi = v.to_string(),
                        "shade" => s.shade = v.to_string(),
                        "mono" => s.mono = flag(v),
                        "lineweights" => s.lineweights = flag(v),
                        "with_styles" => s.with_styles = flag(v),
                        "transparency" => s.transparency = flag(v),
                        "paperspace_last" => s.paperspace_last = flag(v),
                        "hide_paperspace" => s.hide_paperspace = flag(v),
                        "stamp" => s.stamp = flag(v),
                        "save_layout" => s.save_layout = flag(v),
                        "scale_lw" => s.scale_lw = flag(v),
                        _ => {}
                    }
                }
            }
            s
        }
    }

    /// Best-effort persist of the print preferences (silent on failure).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) {
        let Some(path) = prefs_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let b = |v: bool| if v { "1" } else { "0" };
        let printer = self.printer.clone().unwrap_or_default();
        let body = format!(
            "printer={}\nto_file={}\narea={}\nscale={}\ncopies={}\nquality={}\ndpi={}\nshade={}\nmono={}\n\
             lineweights={}\nwith_styles={}\ntransparency={}\npaperspace_last={}\n\
             hide_paperspace={}\nstamp={}\nsave_layout={}\nscale_lw={}\n",
            printer,
            b(self.to_file),
            self.area,
            self.scale,
            self.copies,
            self.quality,
            self.dpi,
            self.shade,
            b(self.mono),
            b(self.lineweights),
            b(self.with_styles),
            b(self.transparency),
            b(self.paperspace_last),
            b(self.hide_paperspace),
            b(self.stamp),
            b(self.save_layout),
            b(self.scale_lw),
        );
        let _ = std::fs::write(path, body);
    }

    /// No-op persist on the web build (no filesystem).
    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) {}
}

#[cfg(not(target_arch = "wasm32"))]
fn prefs_path() -> Option<std::path::PathBuf> {
    Some(crate::config::config_dir()?.join("plot.txt"))
}

fn btn(accent: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, st| button::Style {
        background: Some(Background::Color(match (accent, st) {
            (true, button::Status::Hovered | button::Status::Pressed) => {
                Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 }
            }
            (false, button::Status::Hovered | button::Status::Pressed) => {
                Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 }
            }
            (true, _) => ACCENT,
            _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
        })),
        text_color: TEXT,
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn field_style(_: &Theme, _: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(FIELD),
        border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
        icon: TEXT,
        placeholder: DIM,
        value: TEXT,
        selection: ACCENT,
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

fn section_label<'a>(s: &'static str) -> Element<'a, Message> {
    text(s).size(11).color(DIM).into()
}

/// A `label : dropdown` row. `ctor` turns the picked string into a dialog
/// message.
fn drop_row<'a>(
    label: &'a str,
    options: Vec<String>,
    selected: Option<String>,
    ctor: fn(String) -> PlotDlgMsg,
) -> Element<'a, Message> {
    let pl = pick_list(options, selected, move |s| Message::PlotDlg(ctor(s)))
        .text_size(12)
        .padding([3, 6])
        .width(Length::Fill);
    row![text(label).size(11).color(DIM).width(92), pl]
        .spacing(8)
        .align_y(iced::Center)
        .into()
}

/// A `label : text field` row.
fn field_row<'a>(
    label: &'a str,
    value: &'a str,
    ctor: fn(String) -> PlotDlgMsg,
    width: u16,
) -> Element<'a, Message> {
    row![
        text(label).size(11).color(DIM).width(92),
        text_input("", value)
            .on_input(move |s| Message::PlotDlg(ctor(s)))
            .style(field_style)
            .size(12)
            .width(width as f32),
    ]
    .spacing(8)
    .align_y(iced::Center)
    .into()
}

/// A single option checkbox bound to a `PlotFlag`.
fn check<'a>(label: &'a str, on: bool, flag: PlotFlag) -> Element<'a, Message> {
    checkbox(on)
        .label(label)
        .on_toggle(move |_| Message::PlotDlg(PlotDlgMsg::Flag(flag)))
        .size(14)
        .text_size(11)
        .into()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

pub fn view_window(s: &PlotDialogState) -> Element<'_, Message> {
    // ── Toolbar: Cancel … Preview  Print/Export ──────────────────────────
    let action = if s.to_file { "Export PDF" } else { "Print" };
    let toolbar = container(
        row![
            button(text("Cancel").size(12))
                .on_press(Message::PlotDlg(PlotDlgMsg::Close))
                .style(btn(false))
                .padding([4, 14]),
            Space::new().width(Fill),
            button(text("Preview").size(12))
                .on_press(Message::PlotDlg(PlotDlgMsg::Preview))
                .style(btn(false))
                .padding([4, 12]),
            Space::new().width(8),
            button(text(action).size(12))
                .on_press(Message::PlotDlg(PlotDlgMsg::Commit))
                .style(btn(true))
                .padding([4, 20]),
        ]
        .align_y(iced::Center),
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(TB)),
        ..Default::default()
    })
    .width(Fill)
    .padding([5, 10]);

    // ── Printer dropdown: default + discovered printers + PDF sentinel ────
    let mut printer_opts = vec![OUT_DEFAULT.to_string()];
    printer_opts.extend(s.printers.iter().cloned());
    printer_opts.push(OUT_PDF.to_string());
    let printer_sel = if s.to_file {
        Some(OUT_PDF.to_string())
    } else {
        Some(s.printer.clone().unwrap_or_else(|| OUT_DEFAULT.to_string()))
    };

    let paper_opts: Vec<String> = PaperSize::ALL.iter().map(|p| p.label().to_string()).collect();

    // ── Left column ──────────────────────────────────────────────────────
    let style_label: String = if s.style_name.is_empty() {
        "(none)".into()
    } else {
        s.style_name.clone()
    };
    let left = column![
        section_label("Printer / plotter"),
        drop_row("Output", printer_opts, printer_sel, PlotDlgMsg::Printer),
        field_row("Copies", &s.copies, PlotDlgMsg::Copies, 60),
        hdivider(),
        section_label("Paper"),
        drop_row("Size", paper_opts, Some(s.paper.clone()), PlotDlgMsg::Paper),
        drop_row(
            "Orientation",
            strs(&["Portrait", "Landscape"]),
            Some(s.orientation.clone()),
            PlotDlgMsg::Orientation,
        ),
        drop_row(
            "Rotation",
            strs(&["0°", "90°", "180°", "270°"]),
            Some(s.rotation.clone()),
            PlotDlgMsg::Rotation,
        ),
        hdivider(),
        section_label("Plot area"),
        row![
            container(
                pick_list(
                    strs(&["Layout", "Extents", "Display", "Window"]),
                    Some(s.area.clone()),
                    move |v| Message::PlotDlg(PlotDlgMsg::Area(v)),
                )
                .text_size(12)
                .padding([3, 6])
                .width(Length::Fill)
            )
            .width(Fill),
            button(text("Pick…").size(11))
                .on_press(Message::PlotDlg(PlotDlgMsg::PickWindow))
                .style(btn(false))
                .padding([4, 10]),
        ]
        .spacing(8)
        .align_y(iced::Center),
        section_label("Plot offset"),
        column![
            field_row("X (mm)", &s.offset_x, PlotDlgMsg::OffsetX, 70),
            field_row("Y (mm)", &s.offset_y, PlotDlgMsg::OffsetY, 70),
        ]
        .spacing(9),
        check("Center the plot", s.center, PlotFlag::Center),
    ]
    .spacing(9)
    .width(Fill);

    // ── Right column ─────────────────────────────────────────────────────
    let right = column![
        section_label("Plot scale"),
        drop_row(
            "Scale",
            strs(&["Fit", "1:1", "1:2", "1:5", "1:10", "1:20", "1:50", "1:100", "2:1"]),
            Some(s.scale.clone()),
            PlotDlgMsg::Scale,
        ),
        check("Scale lineweights", s.scale_lw, PlotFlag::ScaleLw),
        hdivider(),
        section_label("Plot style table (pen assignments)"),
        row![
            container(text(style_label).size(12).color(TEXT))
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(FIELD)),
                    border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
                    ..Default::default()
                })
                .padding([4, 8])
                .width(Fill),
            button(text("Load…").size(11))
                .on_press(Message::PlotDlg(PlotDlgMsg::LoadStyle))
                .style(btn(false))
                .padding([4, 10]),
            button(text("Clear").size(11))
                .on_press(Message::PlotDlg(PlotDlgMsg::ClearStyle))
                .style(btn(false))
                .padding([4, 10]),
        ]
        .spacing(6)
        .align_y(iced::Center),
        hdivider(),
        section_label("Quality"),
        row![
            container(
                pick_list(
                    strs(&["Draft", "Normal", "High", "Maximum"]),
                    Some(s.quality.clone()),
                    move |v| Message::PlotDlg(PlotDlgMsg::Quality(v)),
                )
                .text_size(12)
                .padding([3, 6])
                .width(Length::Fill)
            )
            .width(Fill),
            text("DPI").size(11).color(DIM),
            text_input("", &s.dpi)
                .on_input(move |v| Message::PlotDlg(PlotDlgMsg::Dpi(v)))
                .style(field_style)
                .size(12)
                .width(56.0),
        ]
        .spacing(8)
        .align_y(iced::Center),
        drop_row(
            "Shaded plot",
            strs(&["As displayed", "Wireframe", "Hidden", "Rendered"]),
            Some(s.shade.clone()),
            PlotDlgMsg::Shade,
        ),
        hdivider(),
        section_label("Plot options"),
        row![
            column![
                check("Object lineweights", s.lineweights, PlotFlag::Lineweights),
                check("Plot with styles", s.with_styles, PlotFlag::WithStyles),
                check("Monochrome", s.mono, PlotFlag::Mono),
                check("Plot transparency", s.transparency, PlotFlag::Transparency),
            ]
            .spacing(6)
            .width(Fill),
            column![
                check("Paperspace last", s.paperspace_last, PlotFlag::PaperspaceLast),
                check("Hide paperspace", s.hide_paperspace, PlotFlag::HidePaperspace),
                check("Plot stamp", s.stamp, PlotFlag::Stamp),
                check("Save to layout", s.save_layout, PlotFlag::SaveLayout),
            ]
            .spacing(6)
            .width(Fill),
        ]
        .spacing(10),
    ]
    .spacing(9)
    .width(Fill);

    let body = row![left, right].spacing(18).padding(14).width(Fill);
    let content = scrollable(body).width(Fill).height(Fill);

    container(column![toolbar, hdivider(), content].spacing(0))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG)),
            ..Default::default()
        })
        .width(Fill)
        .height(Fill)
        .into()
}
