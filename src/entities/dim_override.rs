//! Per-object dimension-variable overrides.
//!
//! A leader (or dimension) that departs from its dimension style stores the
//! changed variables in the standard `ACAD` XDATA record, identified by a
//! leading `DSTYLE` string, as a list of (dimvar group code, value) pairs wrapped
//! in `{ }` control strings. Both the renderer and the properties panel prefer
//! an override over the style default, so editing one of these rows writes here
//! and the change round-trips to file.

use acadrust::types::Color;
use acadrust::xdata::{ExtendedData, XDataValue};
use acadrust::{CadDocument, Handle};

// DXF group codes of the dimension variables surfaced on the leader panel.
pub const DIMSCALE: i16 = 40; // overall scale       (real)
pub const DIMASZ: i16 = 41; // arrow size          (real)
pub const DIMTAD: i16 = 77; // text vertical pos   (int16)
pub const DIMCLRD: i16 = 176; // dim line colour     (int16 = ACI index)
pub const DIMGAP: i16 = 147; // text offset / gap   (real)
pub const DIMLWD: i16 = 371; // dim line lineweight (int16)
pub const DIMLDRBLK: i16 = 341; // leader arrow block  (handle)

/// Every (code, value) override present in the `ACAD`/`DSTYLE` record.
pub fn pairs(xd: &ExtendedData) -> Vec<(i16, XDataValue)> {
    let values = xd
        .get_record("ACAD")
        .and_then(|rec| match rec.values.first() {
            Some(XDataValue::String(name)) if name == "DSTYLE" => Some(&rec.values[1..]),
            _ => None,
        })
        // Retain compatibility with records produced by older OCS versions.
        .or_else(|| {
            xd.get_record("ACAD_DSTYLE")
                .map(|rec| rec.values.as_slice())
        });
    let Some(values) = values else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut it = values.iter();
    // The record is a flat stream: a 1070 code marker followed by its typed
    // value, bracketed by 1002 "{" / "}" control strings (which are skipped).
    while let Some(v) = it.next() {
        if let XDataValue::Integer16(code) = v {
            if let Some(val) = it.next() {
                out.push((*code, val.clone()));
            }
        }
    }
    out
}

/// The real-valued override for `code`, if present.
pub fn real(xd: &ExtendedData, code: i16) -> Option<f64> {
    pairs(xd)
        .into_iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| match v {
            XDataValue::Real(r) | XDataValue::Distance(r) | XDataValue::ScaleFactor(r) => Some(r),
            _ => None,
        })
}

/// The 16-bit-integer override for `code`, if present.
pub fn int(xd: &ExtendedData, code: i16) -> Option<i16> {
    pairs(xd)
        .into_iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| match v {
            XDataValue::Integer16(n) => Some(n),
            _ => None,
        })
}

/// The colour override for `code`, if present. Dim-colour overrides are stored
/// as an ACI index (the same `int16` slot the dimension style uses), so this
/// decodes it back into a `Color` (0 = ByBlock, 256 = ByLayer, else indexed).
pub fn color(xd: &ExtendedData, code: i16) -> Option<Color> {
    int(xd, code).map(Color::from_index)
}

/// The handle-valued override for `code`, if present.
pub fn handle(xd: &ExtendedData, code: i16) -> Option<Handle> {
    pairs(xd)
        .into_iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| match v {
            XDataValue::Handle(h) => Some(h),
            _ => None,
        })
}

fn write_pairs(doc: &mut CadDocument, handle: Handle, pairs: Vec<(i16, XDataValue)>) {
    let Some(entity) = doc.get_entity(handle) else {
        return;
    };
    let use_canonical_record = entity
        .common()
        .extended_data
        .get_record("ACAD")
        .map(|rec| {
            matches!(
                rec.values.first(),
                Some(XDataValue::String(name)) if name == "DSTYLE"
            )
        })
        .unwrap_or(true);

    let mut values = if pairs.is_empty() {
        None
    } else {
        let mut vals = vec![XDataValue::ControlString("{".to_string())];
        for (code, value) in pairs {
            vals.push(XDataValue::Integer16(code));
            vals.push(value);
        }
        vals.push(XDataValue::ControlString("}".to_string()));
        Some(vals)
    };

    if use_canonical_record {
        if let Some(vals) = &mut values {
            vals.insert(0, XDataValue::String("DSTYLE".to_string()));
        }
        crate::scene::view::dispatch::set_entity_xdata(doc, handle, "ACAD_DSTYLE", None);
        crate::scene::view::dispatch::set_entity_xdata(doc, handle, "ACAD", values);
    } else {
        // Preserve unrelated Autodesk XDATA already occupying the ACAD record.
        crate::scene::view::dispatch::set_entity_xdata(doc, handle, "ACAD_DSTYLE", values);
    }
}

/// Replace every dimension-variable override on entity `handle`.
pub fn replace(doc: &mut CadDocument, handle: Handle, values: Vec<(i16, XDataValue)>) {
    write_pairs(doc, handle, values);
}

/// Set — or, with `value: None`, clear — a single override on entity `handle`,
/// leaving the other overrides in the record untouched. Clearing the last one
/// drops the whole `ACAD`/`DSTYLE` record. Legacy `ACAD_DSTYLE` records written
/// by older OCS versions are migrated when the canonical `ACAD` slot is free.
pub fn set(doc: &mut CadDocument, handle: Handle, code: i16, value: Option<XDataValue>) {
    let Some(entity) = doc.get_entity(handle) else {
        return;
    };
    let mut kept: Vec<(i16, XDataValue)> = pairs(&entity.common().extended_data)
        .into_iter()
        .filter(|(c, _)| *c != code)
        .collect();
    if let Some(v) = value {
        kept.push((code, v));
    }
    write_pairs(doc, handle, kept);
}
