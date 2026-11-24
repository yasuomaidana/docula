# docula-schema

Compiles a JSON Schema into a deterministic finite automaton, masks LLM logits so only schema-valid tokens can be emitted, and defines the `DocuValidate` business-rule trait.

## Key types

`SchemaDfa`, `DfaLogitsMasker`, `DirectiveRegistry`, `FieldDirective`, `DocuValidate`, `ValidationError`, `ValidationMode`

## Specification

- [Module 05: Structured Extraction & Constrained Decoding](../../docs/07_STRUCTURED_EXTRACTION_AND_CONSTRAINED_DECODING.md)

## Internal dependencies

None; this crate has no sibling dependencies.

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
