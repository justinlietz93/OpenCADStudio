use acadrust::{EntityType, Handle};

use crate::scene::object::{PropSection, PropValue, Property};

pub fn general_section(entity: &EntityType) -> PropSection {
    let common = entity.common();
    let linetype_display = if common.linetype.is_empty() {
        "ByLayer".to_string()
    } else {
        common.linetype.clone()
    };
    let transp_pct = (common.transparency.alpha() as f64 / 255.0 * 100.0).round() as u32;

    PropSection {
        title: "General".into(),
        props: vec![
            Property {
                label: "Layer".into(),
                field: "layer",
                value: PropValue::LayerChoice(common.layer.clone()),
            },
            Property {
                label: "Color".into(),
                field: "color",
                value: PropValue::ColorChoice(common.color),
            },
            Property {
                label: "Linetype".into(),
                field: "linetype",
                value: PropValue::LinetypeChoice(linetype_display),
            },
            Property {
                label: "LT Scale".into(),
                field: "linetype_scale",
                value: PropValue::EditText(format!("{:.4}", common.linetype_scale)),
            },
            Property {
                label: "Lineweight".into(),
                field: "lineweight",
                value: PropValue::LwChoice(common.line_weight),
            },
            Property {
                label: "Transparency".into(),
                field: "transparency",
                value: PropValue::EditText(format!("{transp_pct}")),
            },
            Property {
                label: "Invisible".into(),
                field: "invisible",
                value: PropValue::BoolToggle {
                    field: "invisible",
                    value: common.invisible,
                },
            },
        ],
    }
}

pub fn fallback_properties(_handle: Handle, entity: &EntityType) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![Property {
            label: "Type".into(),
            field: "type",
            value: PropValue::ReadOnly(entity_type_name(entity).into()),
        }],
    }
}

fn entity_type_name(e: &EntityType) -> &'static str {
    match e {
        EntityType::Point(_) => "Point",
        EntityType::Line(_) => "Line",
        EntityType::Circle(_) => "Circle",
        EntityType::Arc(_) => "Arc",
        EntityType::Ellipse(_) => "Ellipse",
        EntityType::Spline(_) => "Spline",
        EntityType::LwPolyline(_) => "Polyline",
        EntityType::Polyline(_) => "Polyline",
        EntityType::Polyline2D(_) => "Polyline2D",
        EntityType::Polyline3D(_) => "Polyline3D",
        EntityType::PolyfaceMesh(_) => "PolyfaceMesh",
        EntityType::PolygonMesh(_) => "PolygonMesh",
        EntityType::Text(_) => "Text",
        EntityType::MText(_) => "MText",
        EntityType::Dimension(_) => "Dimension",
        EntityType::Leader(_) => "Leader",
        EntityType::MultiLeader(_) => "MultiLeader",
        EntityType::Tolerance(_) => "Tolerance",
        EntityType::Insert(_) => "Block Reference",
        EntityType::Block(_) => "Block",
        EntityType::BlockEnd(_) => "Block End",
        EntityType::Hatch(_) => "Hatch",
        EntityType::Solid(_) => "Solid",
        EntityType::Face3D(_) => "3D Face",
        EntityType::Solid3D(_) => "3D Solid",
        EntityType::Region(_) => "Region",
        EntityType::Body(_) => "Body",
        EntityType::Mesh(_) => "Mesh",
        EntityType::Ray(_) => "Ray",
        EntityType::XLine(_) => "XLine",
        EntityType::MLine(_) => "MLine",
        EntityType::Viewport(_) => "Viewport",
        EntityType::RasterImage(_) => "Raster Image",
        EntityType::Wipeout(_) => "Wipeout",
        EntityType::Underlay(_) => "Underlay",
        EntityType::Shape(_) => "Shape",
        EntityType::Table(_) => "Table",
        EntityType::AttributeDefinition(_) => "Attribute Definition",
        EntityType::AttributeEntity(_) => "Attribute",
        EntityType::Ole2Frame(_) => "OLE Frame",
        EntityType::Seqend(_) => "Seqend",
        EntityType::Unknown(_) => "Unknown",
    }
}
