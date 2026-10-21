//! Logistics and Shipping Manifest domain schema, macros, and validation.
//!
//! Provides the [`docula_bill_of_lading!`] extensible macro, composable [`ShippingContainer`],
//! [`CargoItem`], and the standard [`BillOfLadingData`] preset.
//! Implements ISO 6346 container checksum verification via [`docula_schema::DocuValidate`].

pub use docula_schema::DocuValidate;
use docula_schema::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Intermodal shipping container details with ISO 6346 identification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShippingContainer {
    /// Standard 11-character ISO 6346 container identifier (e.g. "CSQU3054383").
    #[schemars(description = "11-character ISO 6346 container number (4 letters + 7 digits)")]
    pub container_number: String,

    /// High-security seal serial number.
    #[schemars(description = "Security seal number")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seal_number: Option<String>,

    /// Container ISO size/type code (e.g. "40HC", "20GP", "45G1").
    #[schemars(description = "Container size and type code")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_type: Option<String>,

    /// Tare / payload weight in kilograms.
    #[schemars(description = "Container weight in kg")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_kg: Option<f64>,
}

impl ShippingContainer {
    /// Creates a new shipping container with container number.
    #[must_use]
    pub fn new(container_number: impl Into<String>) -> Self {
        Self {
            container_number: container_number.into(),
            seal_number: None,
            container_type: None,
            weight_kg: None,
        }
    }
}

/// Cargo goods description and physical packaging metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CargoItem {
    /// Commercial description of cargo or commodities.
    #[schemars(description = "Cargo or commodity description")]
    pub description: String,

    /// Number of packages or pieces.
    #[schemars(description = "Quantity of packages")]
    pub quantity: u32,

    /// Packaging format (e.g. "Pallets", "Cartons", "Drums").
    #[schemars(description = "Packaging type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_type: Option<String>,

    /// Gross weight in kilograms.
    #[schemars(description = "Gross weight in kg")]
    pub gross_weight_kg: f64,

    /// Volume in cubic meters (CBM).
    #[schemars(description = "Volume in cubic meters")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_cbm: Option<f64>,

    /// Whether goods are hazardous materials / dangerous goods.
    #[schemars(description = "Hazardous material flag")]
    #[serde(default)]
    pub hazmat: bool,
}

// =========================================================================
// Modular Complexity-Bounded Integrity Validators (CC <= 8, Cognitive <= 8)
// =========================================================================

/// Maps an ISO 6346 character to its numerical check value (skipping multiples of 11).
fn iso_6346_char_value(c: char) -> Option<u32> {
    if c.is_ascii_digit() {
        return c.to_digit(10);
    }
    if c.is_ascii_alphabetic() {
        let upper = c.to_ascii_uppercase();
        let mut val = (upper as u32 - 'A' as u32) + 10;
        if val >= 11 {
            val += 1;
        }
        if val >= 22 {
            val += 1;
        }
        if val >= 33 {
            val += 1;
        }
        return Some(val);
    }
    None
}

/// Validates ISO 6346 container number format and check digit algorithm.
pub fn validate_iso_6346(container_no: &str, field_path: &str) -> Result<(), ValidationError> {
    let clean = container_no.trim();
    if clean.len() != 11 {
        return Err(ValidationError {
            field_path: field_path.to_string(),
            error_message: format!(
                "ISO 6346 container number {} must be exactly 11 characters (e.g. CSQU3054383)",
                clean
            ),
        });
    }

    let chars: Vec<char> = clean.chars().collect();

    // Verify first 4 are letters and next 7 are digits
    if !chars[..4].iter().all(|c| c.is_ascii_alphabetic())
        || !chars[4..].iter().all(|c| c.is_ascii_digit())
    {
        return Err(ValidationError {
            field_path: field_path.to_string(),
            error_message: format!(
                "ISO 6346 container number {} must start with 4 letters followed by 7 digits",
                clean
            ),
        });
    }

    // Check digit calculation
    let mut sum: u32 = 0;
    for (i, &ch) in chars[..10].iter().enumerate() {
        let val = iso_6346_char_value(ch).unwrap_or(0);
        sum += val * (1 << i); // 2^i
    }

    let calculated_check = (sum % 11) % 10;
    let actual_check = chars[10].to_digit(10).unwrap_or(0);

    if calculated_check != actual_check {
        return Err(ValidationError {
            field_path: field_path.to_string(),
            error_message: format!(
                "ISO 6346 container number {} check digit mismatch: expected {}, found {}",
                clean, calculated_check, actual_check
            ),
        });
    }

    Ok(())
}

/// Validates total cargo gross weight against sum of items if present.
pub fn validate_cargo_weight_sum(
    cargo: &[CargoItem],
    total_weight: f64,
    tolerance: f64,
) -> Result<(), ValidationError> {
    if !cargo.is_empty() {
        let items_sum: f64 = cargo.iter().map(|item| item.gross_weight_kg).sum();
        if (items_sum - total_weight).abs() > tolerance {
            return Err(ValidationError {
                field_path: "total_weight_kg".to_string(),
                error_message: format!(
                    "Sum of cargo items gross weight ({:.2} kg) does not match total_weight_kg ({:.2} kg)",
                    items_sum, total_weight
                ),
            });
        }
    }
    Ok(())
}

// =========================================================================
// Macro Generator: docula_bill_of_lading!
// =========================================================================

/// Macro synthesizing customizable Bill of Lading logistics schemas with ISO 6346 validation,
/// intermodal container tracking, and freight routing metadata.
#[macro_export]
macro_rules! docula_bill_of_lading {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $vis struct $name {
            #[schemars(description = "Bill of Lading tracking or booking identifier")]
            pub bol_number: String,

            #[schemars(description = "Ocean or freight carrier line name")]
            pub carrier_name: String,

            #[schemars(description = "Shipper or consignor name and details")]
            pub shipper_name: String,

            #[schemars(description = "Consignee or recipient entity")]
            pub consignee_name: String,

            #[schemars(description = "Commercial vessel or ship name")]
            pub vessel_name: Option<String>,

            #[schemars(description = "Voyage or route reference number")]
            pub voyage_number: Option<String>,

            #[schemars(description = "Port or terminal of origin loading")]
            pub port_of_loading: String,

            #[schemars(description = "Port or terminal of final discharge")]
            pub port_of_discharge: String,

            #[schemars(description = "Assigned shipping containers")]
            pub containers: Vec<$crate::logistics::ShippingContainer>,

            #[schemars(description = "Itemized cargo freight breakdown")]
            pub cargo_items: Vec<$crate::logistics::CargoItem>,

            #[schemars(description = "Total manifest gross weight in kilograms")]
            pub total_weight_kg: f64,

            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_ty,
            )*
        }

        impl $crate::docula_schema::DocuValidate for $name {
            fn validate_integrity(&self) -> Result<(), Vec<$crate::docula_schema::ValidationError>> {
                let mut errors = Vec::new();
                const WEIGHT_TOLERANCE: f64 = 1.0; // 1 kg tolerance

                for (i, c) in self.containers.iter().enumerate() {
                    let field = format!("containers[{}].container_number", i);
                    if let Err(e) = $crate::logistics::validate_iso_6346(&c.container_number, &field) {
                        errors.push(e);
                    }
                }

                if let Err(e) = $crate::logistics::validate_cargo_weight_sum(
                    &self.cargo_items,
                    self.total_weight_kg,
                    WEIGHT_TOLERANCE,
                ) {
                    errors.push(e);
                }

                if self.bol_number.trim().is_empty() {
                    errors.push($crate::docula_schema::ValidationError {
                        field_path: "bol_number".to_string(),
                        error_message: "Bill of Lading number must not be empty".to_string(),
                    });
                }

                if self.carrier_name.trim().is_empty() {
                    errors.push($crate::docula_schema::ValidationError {
                        field_path: "carrier_name".to_string(),
                        error_message: "Carrier name must not be empty".to_string(),
                    });
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    };
}

// Preset standard BillOfLadingData instantiation
docula_bill_of_lading! {
    /// Standardized intermodal shipping manifest and Bill of Lading schema with ISO 6346 checksums.
    pub struct BillOfLadingData {}
}
