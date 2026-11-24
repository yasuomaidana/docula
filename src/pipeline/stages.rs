//! Pipeline stages and facade extensions.

use super::{DoculaPipeline, DoculaPipelineError, ExtractionResult};
use docula_composer::{
    DocumentReconstructor, MultiDocumentSegmenter, ReconstructedDocument, ReconstructionConfig,
    SegmentationConfig, SegmentedSubDocument, SubDocumentIdentity,
};
use docula_core::DocumentSource;
use docula_preprocess::PageRasterBuffer;
use docula_preprocess::filter::PreprocessingFilterChain;
use docula_schema::DocuValidate;
use serde::{Serialize, de::DeserializeOwned};

/// Generates the composer-backed stage wrappers: each builds its composer from an optional
/// config (defaulting when absent) and runs one method on the input.
macro_rules! composer_stage {
    ($(
        $(#[$doc:meta])*
        $name:ident($input:ident: $input_ty:ty, $config_ty:ty)
            => $composer:ident::$method:ident -> $output:ty;
    )+) => {$(
        $(#[$doc])*
        pub fn $name(
            &self,
            $input: $input_ty,
            config: Option<$config_ty>,
        ) -> Result<$output, DoculaPipelineError> {
            Ok($composer::new(config.unwrap_or_default()).$method($input)?)
        }
    )+};
}

impl DoculaPipeline {
    composer_stage! {
        /// **Stage 1 (Image Merging)**: Merges multiple overlapping page raster buffers into a unified reconstructed canvas.
        merge_document_fragments(fragments: &[PageRasterBuffer], ReconstructionConfig)
            => DocumentReconstructor::reconstruct_from_buffers -> ReconstructedDocument;
        /// **Stage 1 (Image Merging)**: Merges multiple overlapping document sources into a unified reconstructed canvas.
        merge_document_source(sources: &[DocumentSource], ReconstructionConfig)
            => DocumentReconstructor::reconstruct_from_sources -> ReconstructedDocument;
        /// **Stage 2 (Instance Segmentation)**: Detects, segments, perspective-rectifies, and fingerprints multiple sub-documents.
        segment_multi_documents(parent: &PageRasterBuffer, SegmentationConfig)
            => MultiDocumentSegmenter::segment_from_buffer -> Vec<SegmentedSubDocument>;
        /// **Stage 2 (Instance Segmentation)**: Detects, segments, perspective-rectifies, and fingerprints multiple sub-documents from source.
        segment_multi_document_source(source: &DocumentSource, SegmentationConfig)
            => MultiDocumentSegmenter::segment_from_source -> Vec<SegmentedSubDocument>;
    }

    /// **Stage 3 (Preprocessing)**: Applies a physical restoration filter chain to an in-memory page buffer.
    pub fn preprocess_buffer(
        &self,
        buffer: &mut PageRasterBuffer,
        filter_chain: Option<&PreprocessingFilterChain>,
    ) -> Result<(), DoculaPipelineError> {
        if let Some(chain) = filter_chain {
            chain
                .process(&mut buffer.image)
                .map_err(DoculaPipelineError::Preprocess)?;
        } else {
            let default_chain = PreprocessingFilterChain::bad_shape_reconstruction_pipeline();
            default_chain
                .process(&mut buffer.image)
                .map_err(DoculaPipelineError::Preprocess)?;
        }
        Ok(())
    }

    /// **Scenario 1 Facade**: Reconstructs a large document from multiple camera shots,
    /// applies physical restoration, and extracts structured data `T`.
    pub async fn reconstruct_and_extract<T>(
        &self,
        fragments: &[DocumentSource],
        config: Option<ReconstructionConfig>,
    ) -> Result<ExtractionResult<T>, DoculaPipelineError>
    where
        T: DocuValidate + schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let merged = self.merge_document_source(fragments, config)?;
        let doc_id = fragments
            .first()
            .map(|s| s.infer_document_id())
            .unwrap_or_else(|| "reconstructed-doc".to_string());
        self.extract_from_buffer::<T>(&doc_id, &merged.buffer).await
    }

    /// **Scenario 2 Facade**: Ingests a single photo containing multiple smaller documents,
    /// identifies each document instance, perspective-rectifies each, and extracts batch schema `T`.
    pub async fn segment_and_extract_batch<T>(
        &self,
        source: &DocumentSource,
        config: Option<SegmentationConfig>,
    ) -> Result<Vec<(SubDocumentIdentity, ExtractionResult<T>)>, DoculaPipelineError>
    where
        T: DocuValidate + schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let subdocs = self.segment_multi_document_source(source, config)?;
        let mut results = Vec::with_capacity(subdocs.len());

        for subdoc in subdocs {
            let ext = self
                .extract_from_buffer::<T>(&subdoc.identity.instance_id, &subdoc.buffer)
                .await?;
            results.push((subdoc.identity, ext));
        }

        Ok(results)
    }
}
