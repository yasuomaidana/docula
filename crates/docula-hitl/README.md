# docula-hitl

Human-in-the-loop triage gates, immutable correction triads, two-tier storage, promotion policies for the active-learning flywheel, and recursive correction-lineage tracking on Diesel CTEs.

## Key types

`PromotionPolicy`, `ThresholdPromotionPolicy`, `CorrectionLineageTracker`, `LineageTableConfig`, `ColdArchiveStore`, `JsonPatchDiff`

## Specification

- [Module 12: Human-in-the-Loop (HITL), Recursive Lineage & Active Learning Flywheel](../../docs/13_HUMAN_IN_THE_LOOP_AND_ACTIVE_LEARNING.md)

## Internal dependencies

`docula-core`, `docula-exemplar`, `docula-ocr`, `docula-schema`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
