# oxipix

`oxipix` is the pure-Rust pixel, mosaic and vectorization crate that `docula` uses. It was designed alongside `docula` and is now a standalone, published crate, so its documentation lives with the crate:

- [crates.io/crates/oxipix](https://crates.io/crates/oxipix): releases and metadata
- [docs.rs/oxipix](https://docs.rs/oxipix): API reference
- [github.com/yasuomaidana/oxipix](https://github.com/yasuomaidana/oxipix): source, architecture and roadmap documents

## How docula uses it

| docula crate | oxipix features | What it uses |
|---|---|---|
| `docula-preprocess` | `mosaic`, `vectorize` | Channel mapping and luma conversion, feathered and pairwise stitching, overlap detection, perspective unwarping, Otsu threshold, SVG vectorization |
| `docula-composer` | `mosaic` | Quad unwarping for multi-document segmentation and the stitching and overlap primitives used by multi-shot reconstruction |

Both depend on it with `default-features = false`, so the GPU and codec features stay off.

The design documents that used to live in this folder (technical assessment, architecture, codec and accelerator engine, roadmap, benchmarks) were moved into the `oxipix` repository when the crate was released. Earlier versions of this folder are in the git history. Why docula keeps the real `image` crate instead of an `oxipix` frame buffer is recorded in [`../21_SOVEREIGN_PIXMAP_AND_ACCELERATOR_EXTIRPATION.md`](../21_SOVEREIGN_PIXMAP_AND_ACCELERATOR_EXTIRPATION.md) and [`../../DEV_STATUS.md`](../../DEV_STATUS.md).
