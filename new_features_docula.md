# Few shot feature

1. Example Preprocessing: When a user provides an example image, run it through your standard image preprocessing and
   OCR pipeline. If the user provides a PDF as an example, use pdf_oxide to render it into an RGBA pixmap before running
   the OCR engine.

2. Template Pairing: Capture the raw OCR text extracted from the example image and pair it with the user's provided JSON
   structure. You can organize and manage these template mappings using a configuration file named Docula.

3. Few-Shot Prompt Construction: When processing a new document, build an LLM prompt that includes the predefined
   examples. The prompt should instruct the LLM, provide the example OCR text, show the expected JSON output, and
   finally provide the raw OCR text of the new document for the LLM to process.

4. Format Generalization: Because the LLM uses the provided examples purely as a pattern-matching guide rather than
   strict hardcoded rules, the system will retain its ability to process diverse, unseen formats, multiple languages,
   and varying multi-page PDFs.

To ensure the system remains performant, design the prompt builder so that these few-shot examples are strictly
optional. When omitted, the LLM can fall back to zero-shot extraction relying entirely on its internal instruction
tuning, which will save context tokens and reduce inference latency for standard document parsing.

# Human in the loop

In modern Document AI pipelines, handling edge cases and incorrect extractions rely on an active learning flywheel
structured into four stages:

```
[Inference Pipeline] ──► [Confidence & Rule Gates] ──┬──► Valid ──► Downstream Systems
                                                     │
                                                     └──► Failed/Rejected
                                                              │
                                                              ▼
                                                     [Review Queue (HITL)]
                                                              │
                                                              ▼
                                                   [Curated Triad Ingestion]
```

1. Automated Triage Gates: Run the LLM's output through deterministic checks (such as verifying invoice arithmetic
   subtotal + tax == total or enforcing schema invariants via serde) before surfacing it to end users. If parsing fails
   or
   confidence drops below a threshold, flag the document for human triage.

2. Correction Triad Capture: When a human reviews and amends the data, capture the correction as an immutable tuple:

    - Source artifact: Original document image/PDF or its stable cryptographic hash.

    - Intermediate input: The raw OCR spatial text (or text extracted via pdf_oxide).

    - Model output (Bad): The erroneous JSON output and specific error annotations.

    - Ground truth (Corrected): The final validated JSON payload confirmed by the user.

    - Metadata: Vendor identifier, layout template hash, and user edit diffs.

3. Dynamic Exemplar Retrieval: Rather than passing all historical corrections into every prompt, index the corrected
   examples by vendor, document layout, or vector similarity. When a similar document arrives in the future, retrieve
   the
   top 1–2 most relevant corrected exemplars to inject into the few-shot prompt context.

Do not store raw corrections directly into the active examples/ storage directory. Unchecked dumping of corrections into
your few-shot pool creates serious performance and quality risks:

- Prompt bloat and latency: LLM context windows become oversaturated with redundant examples, driving up inference
  latency
  and compute overhead.

- Prompt poisoning and edge-case overfitting: A one-off user mistake or conflicting document format in examples/ can
  misguide the LLM on otherwise standard documents.

The recommended storage pattern:

Audit & Retraining Archive (/data/corrections/ or cold storage): Store all correction triads here as immutable JSONL or
Parquet records. This serves as your benchmark regression test suite and future fine-tuning dataset.

Curated Few-Shot Pool (/data/examples/): Maintain a distinct, curated directory containing high-value, deduplicated
exemplars. Promote a correction from the archive to the active few-shot pool only if it represents a recurring layout,
fixes a persistent edge case, and passes validation.

## Edge Computing and Regulatory compliance

Invoices, payroll documents, and bank statements contain sensitive financial data and Personally Identifiable
Information (PII). Deploying this crate on the edge—such as on-premises servers, branch appliances, or local client
machines—significantly strengthens data compliance (e.g., GDPR, CCPA, SOC 2, PCI-DSS):

- Zero Data Egress: Compiling to a pure-Rust binary allows the entire OCR, rendering via pdf_oxide, and local LLM
  execution to occur strictly within the client’s private boundary. Unprocessed source images and raw bank details never
  touch external cloud APIs.

- Local PII Redaction for Exemplar Sharing: If your architecture needs to synchronize useful layout examples back to a
  central pool, edge processing enables deterministic PII sanitization (masking IBANs, names, and tax IDs while
  preserving
  geometric bounding boxes and JSON keys) before records leave the local perimeter.

- Cryptographic Data Minimization and TTL: Edge nodes can apply strict retention policies directly: persist raw images
  only for the duration of a human review window (e.g., 24 hours), then discard pixel buffers and retain only encrypted
  schema diffs.

## Handling non alphanumeric fields

Reusing the spatial redaction logic from your compliance workflow is the ideal approach for handling non-textual data
like QR codes and barcodes. Because your edge compliance strategy already relies on isolating geometric bounding boxes
and JSON keys to mask sensitive data like IBANs or tax IDs, you can expand this foundation into a generalized
region-of-interest (ROI) ruleset.

You can implement this by adding declarative action tags directly into your target JSON schema or your Docula
configuration file. When the pipeline processes a document, it maps these tags to the spatial coordinates identified
during the extraction phase.

Here is how you can structure the execution across your pipeline:

Detection & Coordinate Mapping: Before generating the LLM prompt, run the rendered image (produced via pdf_oxide)
through a pure-Rust barcode/QR detection crate to capture the bounding boxes and raw payloads of any optical codes. This
creates a unified spatial map combining standard OCR text and non-text bounding boxes.

Action Routing via Schema Tags: Add specific processing directives to the fields in your expected JSON structure to
dictate how the mapped coordinates should be handled:

@decode: The system extracts the optical code's payload (e.g., a URL) and injects it into the text layer as a string for
the LLM to contextualize, discarding the image pixels.

@mask: The system uses the bounding box coordinates to crop the exact region of the QR/barcode, encoding the raw visual
artifact (e.g., as a base64 string or an isolated image reference) while ignoring the decoded data.

@censor or @ignore: The system actively blacks out the pixels within that bounding box on the source image and prevents
any text inside that region from being passed to the LLM.

Human-in-the-Loop (HITL) & Storage Interception: When a document is flagged for human triage and enters the correction
triad capture process, the pipeline applies these compliance tags deterministically. If a section is marked @censor or
@ignore, the system permanently redacts those geometric boundaries from the source artifact before it is displayed to
the reviewer or saved to the immutable /data/corrections/ archive.

By applying these rules at the bounding-box level during preprocessing, the LLM and the human reviewers only ever
interact with the exact representation of the data (masked, decoded, or scrubbed) dictated by your schema. This
automatically ensures that your curated few-shot exemplars promoted to the /data/examples/ directory remain fully
compliant without requiring secondary manual sanitization.