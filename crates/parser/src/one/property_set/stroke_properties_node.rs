use crate::errors::Result;
use crate::one::property::ink_dimensions::InkDimension;
use crate::one::property::{PropertyType, simple};
use crate::one::property_set::{PropertySetId, assert_property_set};
use crate::onestore::Object;

/// An ink stroke's properties.
#[allow(dead_code)]
pub(crate) struct Data {
    pub(crate) aliased: bool,
    pub(crate) fit_to_curve: bool,
    pub(crate) ignore_pressure: bool,
    pub(crate) pen_tip: Option<u8>,
    pub(crate) raster_operation: Option<u8>,
    pub(crate) transparency: Option<u8>,
    pub(crate) ink_height: f32,
    pub(crate) ink_width: f32,
    pub(crate) color: Option<u32>,
    pub(crate) dimensions: Vec<InkDimension>,
}

pub(crate) fn parse(object: &Object) -> Result<Data> {
    assert_property_set(object, PropertySetId::StrokePropertiesNode)?;

    let aliased = simple::parse_bool(PropertyType::InkAntialised, object)?.unwrap_or_default();
    let fit_to_curve = simple::parse_bool(PropertyType::InkFitToCurve, object)?.unwrap_or_default();
    let ignore_pressure =
        simple::parse_bool(PropertyType::InkIgnorePressure, object)?.unwrap_or_default();
    let pen_tip = simple::parse_u8(PropertyType::InkPenTip, object)?;
    let raster_operation = simple::parse_u8(PropertyType::InkRasterOperation, object)?;
    let transparency = simple::parse_u8(PropertyType::InkTransparency, object)?;
    // A stroke can lack one or both pen sizes (seen in a section that went through several
    // OneNote versions). Use the other size, else a default, instead of failing the whole section.
    let height = simple::parse_f32(PropertyType::InkHeight, object)?;
    let width = simple::parse_f32(PropertyType::InkWidth, object)?;
    let ink_width = height.or(width).unwrap_or_else(|| warn_missing_size("height"));
    let ink_height = width.or(height).unwrap_or_else(|| warn_missing_size("width"));
    let color = simple::parse_u32(PropertyType::InkColor, object)?;
    let dimensions = InkDimension::parse(PropertyType::InkDimensions, object)?;

    Ok(Data {
        aliased,
        fit_to_curve,
        ignore_pressure,
        pen_tip,
        raster_operation,
        transparency,
        ink_height,
        ink_width,
        color,
        dimensions,
    })
}

const DEFAULT_PEN_SIZE: f32 = 1.0;

// TODO: thread a `ParserContext` through `parse` and switch this to the
// crate-level `warn!(ctx, ...)` macro.
#[allow(clippy::disallowed_macros)]
fn warn_missing_size(which: &str) -> f32 {
    log::warn!("ink stroke properties has no pen {which}, using {DEFAULT_PEN_SIZE}");
    DEFAULT_PEN_SIZE
}
