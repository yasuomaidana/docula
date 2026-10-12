# Module 06: End-to-End Invoice & Generic Schema Examples

## 1. Overview & Purpose

This document provides complete, practical code walkthroughs showing how to use `Docula` to extract structured data into user-defined Rust types.

We demonstrate:
1. Extracting the **Invoice Data Structure** from `starting_point.md`.
2. Processing diverse input types: Single PDF, Scanned Image, and a Multi-Image Batch.
3. Defining custom schemas (e.g. `BankStatementData`) with zero boilerplate.

---

## 2. The Invoice Data Structure Definition

Users define their target document schema using standard Rust structs decorated with `schemars::JsonSchema` and `serde::{Serialize, Deserialize}`. Field-level doc comments and `#[schemars(description = "...")]` attributes provide semantic guidance to the underlying LLM:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InvoiceData {
    #[schemars(description = "The legal name of the vendor or merchant")]
    pub vendor_name: String,

    #[schemars(description = "Invoice issue date in ISO-8601 format (YYYY-MM-DD)")]
    pub date: String,

    #[schemars(description = "List of individual purchased items or services")]
    pub line_items: Vec<LineItem>,

    #[schemars(description = "Grand total amount due including taxes and fees")]
    pub total_amount: f64,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LineItem {
    #[schemars(description = "Description or title of the product/service")]
    pub description: String,

    #[schemars(description = "Quantity purchased")]
    pub quantity: f64,

    #[schemars(description = "Unit price per item")]
    pub unit_price: f64,

    #[schemars(description = "Total line price (quantity * unit_price)")]
    pub total: f64,
}

// =========================================================================
// OPTIONAL: Implement DocuValidate if you want internal integrity checks
// If omitted, Docula will simply return the raw output directly from Step 2.
// =========================================================================
use Docula::{DocuValidate, ValidationError};

impl DocuValidate for InvoiceData {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // 1. Arithmetic integrity: Check that line items sum to grand total (within $0.05 tolerance for rounding)
        let calculated_sum: f64 = self.line_items.iter().map(|item| item.total).sum();
        if (calculated_sum - self.total_amount).abs() > 0.05 {
            errors.push(ValidationError {
                field_path: "total_amount".into(),
                error_message: format!(
                    "Sum of line items ({:.2}) does not match grand total ({:.2})",
                    calculated_sum, self.total_amount
                ),
            });
        }

        // 2. Line item integrity: Check that quantity * unit_price == total
        for (i, item) in self.line_items.iter().enumerate() {
            let expected_total = item.quantity * item.unit_price;
            if (expected_total - item.total).abs() > 0.05 {
                errors.push(ValidationError {
                    field_path: format!("line_items[{}].total", i),
                    error_message: format!(
                        "Item '{}' line total ({:.2}) does not equal quantity ({}) * unit_price ({:.2})",
                        item.description, item.total, item.quantity, item.unit_price
                    ),
                });
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

---

## 3. Practical Usage Examples

### Example A: Extracting from a PDF File (With Optional Validation)

```rust
use Docula::{extract_structure, DocumentSource, OxiBonsaiBackend};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize local embedded LLM backend (zero-FFI, 1-bit quantized)
    let backend = OxiBonsaiBackend::load_default().await?;

    // 2. Specify the PDF document
    let pdf_source = DocumentSource::PdfPath(PathBuf::from("data/invoices/vendor_invoice_1042.pdf"));

    // 3. Execute the two-step extraction into our typed InvoiceData struct
    let invoice: InvoiceData = extract_structure(pdf_source, &backend).await?;

    println!("Successfully extracted invoice from PDF!");
    println!("Vendor: {}", invoice.vendor_name);
    println!("Date: {}", invoice.date);
    println!("Total Amount: ${:.2}", invoice.total_amount);
    for (i, item) in invoice.line_items.iter().enumerate() {
        println!("  Item {}: {} x {} @ ${:.2} = ${:.2}",
            i + 1, item.quantity, item.description, item.unit_price, item.total
        );
    }

    Ok(())
}
```

---

### Example B: Extracting from a Single Picture / Camera Capture

```rust
use Docula::{extract_structure, DocumentSource, OxiBonsaiBackend};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = OxiBonsaiBackend::load_default().await?;

    // Ingest a scanned JPEG or camera receipt photo
    let img_source = DocumentSource::ImagePath(PathBuf::from("data/receipts/gas_station_receipt.jpg"));

    // Ingest and extract automatically applying deskewing, binarization, PP-OCRv6, and structured decoding
    let invoice: InvoiceData = extract_structure(img_source, &backend).await?;

    println!("Extracted from Receipt Image: {:#?}", invoice);
    Ok(())
}
```

---

### Example C: Extracting from a Set of Pictures (Multi-Page Photo Batch)

In many operational scenarios, a user captures multiple consecutive photos of a long multi-page invoice or stapled document packet:

```rust
use Docula::{extract_structure, DocumentSource, OxiBonsaiBackend};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = OxiBonsaiBackend::load_default().await?;

    // Provide a set / vector of pictures
    let multi_image_batch = DocumentSource::ImageBatch(vec![
        DocumentSource::ImagePath(PathBuf::from("data/multi_page/page_1.png")),
        DocumentSource::ImagePath(PathBuf::from("data/multi_page/page_2.png")),
        DocumentSource::ImagePath(PathBuf::from("data/multi_page/page_3.png")),
    ]);

    // The engine processes each image in parallel across CPU cores via Rayon,
    // joins their spatial layouts, and feeds the unified document context into Step 2.
    let invoice: InvoiceData = extract_structure(multi_image_batch, &backend).await?;

    println!("Extracted multi-page invoice with {} line items!", invoice.line_items.len());
    Ok(())
}
```

---

## 4. Defining Generic Custom Schemas

The engine is completely generic and works with any schema conforming to `JsonSchema + DeserializeOwned`.

### Example: Bank Statement Extraction

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct BankStatement {
    #[schemars(description = "Financial institution name")]
    pub bank_name: String,

    #[schemars(description = "Account number or masked account ID")]
    pub account_number: String,

    #[schemars(description = "Statement period start date")]
    pub period_start: String,

    #[schemars(description = "Statement period end date")]
    pub period_end: String,

    #[schemars(description = "Opening statement balance")]
    pub opening_balance: f64,

    #[schemars(description = "Closing statement balance")]
    pub closing_balance: f64,

    #[schemars(description = "List of posted transactions")]
    pub transactions: Vec<BankTransaction>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct BankTransaction {
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    Debit,
    Credit,
}
```

### Extracting Unvalidated Schemas (Raw Output)

Notice that `BankStatement` does **not** implement `DocuValidate`. In this case, `Docula` executes the 2-step extraction and returns the raw output immediately without performing any validation passes:

```rust
// Simply call extract_structure; raw deserialized output is returned directly
let statement: BankStatement = extract_structure(doc_source, &backend).await?;
println!("Extracted raw bank statement: {} (Transactions: {})", statement.bank_name, statement.transactions.len());
```
