# docula-ocr

Multi-lingual PP-OCRv6 text detection (DBNet), character recognition (CRNN), language asset management and spatial reading-order clustering.

## Status

Detection is heuristic (Otsu threshold plus connected components) and recognition returns pre-registered text per region. Neural PP-OCRv6 inference is specified but not implemented yet.

## Key types

`OcrEngine`, `NeuralOcrEngine`, `MockOcrEngine`, `DbNetDetector`, `CrnnRecognizer`, `LanguageAssetManager`, `TextLine`

## Specification

- [Module 03: Multi-Lingual OCR & Spatial Extraction](../../docs/05_OCR_AND_SPATIAL_EXTRACTION.md)
- [Module 16: Multi-Lingual OCR & Language Asset Management](../../docs/16_MULTI_LINGUAL_OCR_AND_ASSET_MANAGEMENT.md)

## Internal dependencies

`docula-core`, `docula-preprocess`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
