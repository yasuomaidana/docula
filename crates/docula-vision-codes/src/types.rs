use docula_core::{BoundingBox, Point};
use serde::{Deserialize, Serialize};

/// Classification of optical 1D and 2D barcode symbols.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpticalCodeType {
    /// 2D QR Code.
    QrCode,
    /// 2D Data Matrix.
    DataMatrix,
    /// 2D Aztec code.
    Aztec,
    /// 2D / Stacked PDF417 code.
    Pdf417,
    /// 1D Code 128 barcode.
    Code128,
    /// 1D Code 39 barcode.
    Code39,
    /// 1D Code 93 barcode.
    Code93,
    /// 1D EAN-13 European Article Number.
    Ean13,
    /// 1D EAN-8 European Article Number.
    Ean8,
    /// 1D UPC-A Universal Product Code.
    UpcA,
    /// 1D UPC-E Universal Product Code.
    UpcE,
    /// 1D Codabar barcode.
    Codabar,
    /// 1D Interleaved 2 of 5.
    Itf,
    /// Other / unrecognized barcode format.
    Other(String),
}

/// Parsed European Payments Council (EPC) SEPA Quick Response payment payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SepaEpcPayload {
    /// Service tag, standard specifies "BCD".
    pub service_tag: String,
    /// Version code, e.g. "001" or "002".
    pub version: String,
    /// Character set coding, e.g. "1" (UTF-8) or "2" (ISO 8859-1).
    pub character_set: String,
    /// Identification code, standard specifies "SCT" (SEPA Credit Transfer).
    pub identification: String,
    /// Bank Identifier Code (BIC) of the Beneficiary Bank.
    pub bic: String,
    /// Name of the Beneficiary.
    pub beneficiary_name: String,
    /// International Bank Account Number (IBAN) of the Beneficiary.
    pub iban: String,
    /// Transfer amount, if specified.
    pub amount: Option<f64>,
    /// Currency code (defaults to "EUR").
    pub currency: String,
    /// Purpose code (4 characters), if specified.
    pub purpose: Option<String>,
    /// Structured Remittance Reference (e.g. ISO 11649 RF Creditor Reference).
    pub remittance_reference: Option<String>,
    /// Unstructured Remittance Information text.
    pub remittance_text: Option<String>,
}

/// Parsed Swiss QR-bill payment payload (SPC - Swiss Payments Code).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwissQrPayload {
    /// QR-bill specification version (e.g. "0200").
    pub version: String,
    /// Currency code ("CHF" or "EUR").
    pub currency: String,
    /// Payment amount, if specified.
    pub amount: Option<f64>,
    /// QR-IBAN or standard IBAN of the creditor.
    pub iban: String,
    /// Name of the creditor.
    pub creditor_name: String,
    /// Address or town of the creditor.
    pub creditor_address: String,
    /// Payment reference (QR reference or Creditor Reference).
    pub reference: Option<String>,
    /// Unstructured message / bill information.
    pub message: Option<String>,
}

/// Parsed Mexican SAT CFDI electronic invoice verification QR payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MexicanSatCfdiPayload {
    /// Fiscal Folio UUID (`id` parameter).
    pub uuid: String,
    /// RFC of the issuer (`re` parameter).
    pub rfc_issuer: String,
    /// RFC of the receiver (`rr` parameter).
    pub rfc_receiver: String,
    /// Total invoice amount (`tt` parameter).
    pub total: f64,
    /// Digital stamp signature hash tail (`fe` parameter).
    pub fe: String,
}

/// Normalized financial payment or invoice payload decoded from a 2D optical code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FinancialQrPayload {
    /// European EPC SEPA credit transfer QR.
    SepaEpc(SepaEpcPayload),
    /// Swiss QR-bill payment code.
    SwissQr(SwissQrPayload),
    /// Mexican SAT CFDI electronic invoice verification URL.
    MexicanSatCfdi(MexicanSatCfdiPayload),
    /// Raw unparsed financial payload.
    Raw(String),
}

/// Fully decoded optical code mapped to document coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpticalCodeResult {
    /// Format and symbology of the optical code.
    pub code_type: OpticalCodeType,
    /// Raw decoded string payload.
    pub raw_content: String,
    /// Normalized page bounding box [0.0..1.0].
    pub bounding_box: BoundingBox,
    /// Corner/finder pattern vertices in normalized page coordinates.
    pub points: Vec<Point>,
    /// Structured financial metadata, if recognized.
    pub financial_payload: Option<FinancialQrPayload>,
}
