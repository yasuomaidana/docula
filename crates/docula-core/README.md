# docula-core

Core primitives for the docula engine: normalized `[0.0, 1.0]` geometry, the universal `DocumentSource`, the layered `DoculaConfig` loader (defaults, TOML, environment) and the zero-storage `NonStored<T>` wrapper that zeroizes PII on drop.

## Key types

`Point`, `DocumentSource`, `DoculaConfig`, `DoculaConfigBuilder`, `NonStored<T>`, `ImageCrop`, `RegionCropper`, `DoculaError`

## Specification

- [Module 15: Consumer Configuration Strategy & TOML Integration](../../docs/15_CONSUMER_CONFIGURATION_AND_TOML.md)
- [Module 13: Edge Compliance, Non-Storing Directives & Data Redaction](../../docs/14_COMPLIANCE_AND_NON_STORING.md)
- [Module 18: Document Screenshots & Visual Region Cropping](../../docs/18_DOCUMENT_SCREENSHOTS_AND_REGION_CROPPING.md)

## Internal dependencies

None; this crate has no sibling dependencies.

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
