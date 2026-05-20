use std::cell::Cell;

use glam::Vec3;

use crate::scene::object::{GripDef, GripShape, PropValue, Property};

/// Linear / angular unit format pulled from the document header so the
/// per-thread properties pipeline can format values consistently without
/// passing the document through every callsite.
#[derive(Clone, Copy, Default)]
pub struct UnitContext {
    /// LUNITS — 1=Sci, 2=Decimal, 3=Engineering, 4=Architectural, 5=Fractional
    pub lunits: i16,
    /// LUPREC — decimal places (linear)
    pub luprec: i16,
    /// AUNITS — 0=Decimal degrees, 1=DMS, 2=Grad, 3=Rad. Surfaced via
    /// `format_angle`, which is read on demand by code that already
    /// formats angular values via radians.
    #[allow(dead_code)]
    pub aunits: i16,
    /// AUPREC — decimal places (angular)
    #[allow(dead_code)]
    pub auprec: i16,
}

thread_local! {
    static UNIT_CTX: Cell<UnitContext> = const { Cell::new(UnitContext {
        lunits: 2,
        luprec: 4,
        aunits: 0,
        auprec: 0,
    }) };
}

/// Set the per-thread unit context. Properties helpers consult it when
/// they format f64 values into display strings.
pub fn set_unit_context(ctx: UnitContext) {
    UNIT_CTX.with(|c| c.set(ctx));
}

pub fn unit_context() -> UnitContext {
    UNIT_CTX.with(|c| c.get())
}

/// Format a linear length using LUNITS / LUPREC. Architectural / fractional
/// produce "n'-d/D"" style strings (1 unit = 1 inch); decimal / scientific /
/// engineering / Windows-desktop fall back to plain decimal at LUPREC places.
pub fn format_length(value: f64) -> String {
    let ctx = unit_context();
    let prec = ctx.luprec.max(0) as usize;
    match ctx.lunits {
        1 => format!("{:.*e}", prec, value),
        3 => {
            // Engineering: ft-inches, decimal inches.
            let sign = if value < 0.0 { "-" } else { "" };
            let abs = value.abs();
            let feet = (abs / 12.0).trunc();
            let rem = abs - feet * 12.0;
            format!("{}{:.0}'-{:.*}\"", sign, feet, prec, rem)
        }
        4 | 5 => {
            // Architectural / Fractional — n + fraction with 1/2^p denom (1
            // unit = 1 inch). Use 6 as a moderate denominator power so the
            // result reads like 1/64".
            let sign = if value < 0.0 { "-" } else { "" };
            let abs = value.abs();
            let (feet, in_rem) = if ctx.lunits == 4 {
                let f = (abs / 12.0).trunc();
                (Some(f as i64), abs - f * 12.0)
            } else {
                (None, abs)
            };
            let whole = in_rem.trunc();
            let frac = in_rem - whole;
            let denom = 64u64;
            let numer = (frac * denom as f64).round() as i64;
            let mut n = numer as u64;
            let mut d = denom;
            while d > 1 && n % 2 == 0 && d % 2 == 0 {
                n /= 2;
                d /= 2;
            }
            let frac_str = if n == 0 || d == 1 {
                String::new()
            } else {
                format!(" {}/{}", n, d)
            };
            let unit_suffix = if ctx.lunits == 4 { "\"" } else { "" };
            match feet {
                Some(f) => format!("{}{}'-{:.0}{}{}", sign, f, whole, frac_str, unit_suffix),
                None => format!("{}{:.0}{}", sign, whole, frac_str),
            }
        }
        _ => format!("{:.*}", prec, value),
    }
}

/// Format an angle (input in radians) using AUNITS / AUPREC.
#[allow(dead_code)]
pub fn format_angle(value_rad: f64) -> String {
    let ctx = unit_context();
    let prec = ctx.auprec.max(0) as usize;
    match ctx.aunits {
        1 => {
            // DMS — degrees / minutes / seconds.
            let deg = value_rad.to_degrees();
            let sign = if deg < 0.0 { "-" } else { "" };
            let a = deg.abs();
            let d = a.floor();
            let m_full = (a - d) * 60.0;
            let m = m_full.floor();
            let s = (m_full - m) * 60.0;
            format!("{}{:.0}°{:.0}'{:.*}\"", sign, d, m, prec, s)
        }
        2 => {
            let g = value_rad.to_degrees() / 0.9;
            format!("{:.*}g", prec, g)
        }
        3 => format!("{:.*}r", prec, value_rad),
        _ => format!("{:.*}°", prec, value_rad.to_degrees()),
    }
}

pub fn square_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Square,
    }
}

pub fn diamond_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: true,
        shape: GripShape::Diamond,
    }
}

pub fn triangle_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Triangle,
    }
}

pub fn edit_prop(label: &'static str, field: &'static str, value: f64) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::EditText(format_length(value)),
    }
}

pub fn ro_prop(label: &'static str, field: &'static str, value: impl Into<String>) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::ReadOnly(value.into()),
    }
}

pub fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}
