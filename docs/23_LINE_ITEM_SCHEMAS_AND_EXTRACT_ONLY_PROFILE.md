# Module 23: Line-Item Schemas, Authoritative Sources & Extract-Only Profile

## 1. Purpose

This module adds richer line-item extraction for invoices, a generic schema for itemised documents that refer to another document, a way to mark a machine-readable original as authoritative, and a build profile for callers that need extraction only. It builds on Module 22 (real recognition backends, fixed-point amounts, per-field positions), which must be complete first. No tickets exist yet; this is the requirement and its acceptance criteria.

## 2. Assessment of the Current Implementation

1. The invoice schema captures totals well but line items only loosely: there is no line position, issuer item code, unit, or per-line tax breakdown, and no field for a reference to a related document.
2. Itemised documents that are not invoices (a list of items with quantities and a reference to another document, with no prices) have no schema.
3. When a machine-readable original accompanies a scan (for example a signed XML), there is no way to mark it authoritative so that extraction is skipped or used only for cross-checking.
4. A caller that wants extraction without retrieval or model crates cannot express that with one feature set; some optional crates are pulled in by default features.

## 3. Requirements

| ID | Priority | Crate | Requirement |
|---|---|---|---|
| `ITEM-1` | high | `docula-domains` | Invoice line items with position, item code, quantity, unit, unit price, tax lines, and a related-document reference |
| `ITEM-2` | medium | `docula-domains` | Generic itemised-document schema (items, quantities, optional printed code, related-document reference) |
| `ITEM-3` | medium | `docula` | `authoritative_source` input: structured original wins; extraction result is compared and differences reported |
| `ITEM-4` | high | `docula` | `extract-only` feature profile: no retrieval, no model crates in the dependency tree |

## 4. Specifications

### ITEM-1: Invoice line items

**Request:** Fields: issuer identity and tax id, recipient tax id, document number and date, related-document reference, currency, per line (position, item code, description, quantity, unit, unit price, tax lines, line total), document totals, payment terms. Amounts are fixed-point integers. Each field carries page, position, and confidence.

**Acceptance criteria:** on a sample set of at least 20 invoices, line count is correct on 95 % and totals reconcile exactly (lines + taxes = total) or the result is flagged; no amount is parsed through floating point.

### ITEM-2: Itemised document

**Request:** Issuer, date, related-document reference, per line (item, quantity, unit, printed code if present, optional date).

**Acceptance criteria:** schema validation rejects a document with no related-document reference unless explicitly allowed; printed codes are preserved verbatim.

### ITEM-3: Authoritative structured source

**Request:** The extraction entry point accepts an optional structured original. When present, its values populate the result; image extraction runs only if requested, and field-level differences are returned as a comparison report.

**Acceptance criteria:** with a structured original present, the result equals the original's values regardless of image quality; differences are listed with both values and positions.

### ITEM-4: Extract-only profile

**Request:** A feature `extract-only` (and documentation of it) under which the dependency tree contains no retrieval or model-inference crate.

**Acceptance criteria:** `cargo tree -e normal --no-default-features --features extract-only` lists none of them; a CI job asserts this.
