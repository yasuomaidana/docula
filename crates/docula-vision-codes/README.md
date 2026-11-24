# docula-vision-codes

Pure-Rust optical code detection (QR and 1D barcodes via `rxing`) and parsing of financial payloads: SEPA EPC, Swiss QR and Mexican SAT CFDI.

## Key types

`OpticalCodeDetector`, `OpticalCodeType`, `FinancialQrPayload`, `SepaEpcPayload`, `SwissQrPayload`, `MexicanSatCfdiPayload`, `parse_financial_payload`

## Specification

- [Module 17: Pure-Rust Optical Code Extraction (QR & Barcodes)](../../docs/17_OPTICAL_CODE_EXTRACTION.md)

## Internal dependencies

`docula-core`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
