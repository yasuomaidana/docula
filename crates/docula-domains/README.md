# docula-domains

Standard vertical schemas with arithmetic integrity rules (`DocuValidate`) and extension macros: invoices (with SEPA, CFDI and Swiss QR adapters), receipts, bank statements and bills of lading.

## Key types

`InvoiceData`, `ReceiptData`, `BankStatementData`, `BillOfLadingData`, `LineItem`, `DiscountPolicy`, `docula_invoice!`, `docula_receipt!`, `docula_bank_statement!`, `docula_bill_of_lading!`

## Specification

- [Module 06: End-to-End Invoice & Generic Schema Examples](../../docs/08_INVOICE_AND_GENERIC_SCHEMA_EXAMPLE.md)
- [Module 22: Finance Document Extraction & Real Inference Backends](../../docs/22_FINANCE_DOCUMENT_EXTRACTION_AND_REAL_INFERENCE.md)
- [Module 23: Line-Item Schemas, Authoritative Sources & Extract-Only Profile](../../docs/23_LINE_ITEM_SCHEMAS_AND_EXTRACT_ONLY_PROFILE.md)

## Internal dependencies

`docula-schema`, `docula-vision-codes`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
