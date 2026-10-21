# Architecture & Design Patterns Specification: `oxipix`

## 1. Persona & Structural Architecture

`oxipix` is architected under strict **Principal Software Architect** and **Code Quality Auditor** guidelines to ensure idiomatic Rust ergonomics, deterministic memory behavior, zero unverified assumptions, and low algorithmic complexity.

---

## 2. Core Rust Design Patterns

### 2.1 Typestate Pattern for Buffer Lifecycle & State Validation
Buffers transition through well-defined lifecycle states validated at compile time via zero-sized phantom types (`ZST`). This prevents invalid operations (such as reading uninitialized pixel memory or attempting GPU dispatch on un-aligned buffers) without runtime branch overhead.

```rust
pub mod state {
    /// Buffer memory has been allocated but pixel contents are uninitialized.
    pub struct Uninitialized;
    /// Buffer contains verified pixel data ready for CPU read/write operations.
    pub struct Initialized;
    /// Buffer is immutable, frozen, and shared across threads via Arc ref-counting.
    pub struct Shared;
    /// Buffer memory has been mapped into GPU-accessible staging memory.
    pub struct GpuStaged;
}

/// Generic sovereign frame buffer parameterized by pixel format and typestate.
pub struct FrameBuffer<Format, State = state::Initialized> {
    width: PixelWidth,
    height: PixelHeight,
    stride: PixelStride,
    storage: StorageBacking,
    _format: std::marker::PhantomData<Format>,
    _state: std::marker::PhantomData<State>,
}

impl<Format> FrameBuffer<Format, state::Uninitialized> {
    /// Allocates a new 64-byte aligned uninitialized buffer.
    pub fn allocate(width: PixelWidth, height: PixelHeight) -> Result<Self, PixError> {
        // Validation logic (CC <= 5)
    }

    /// Initializes buffer memory with deterministic pixel values, transitioning to `Initialized`.
    pub fn fill(mut self, pixel: Format) -> FrameBuffer<Format, state::Initialized> {
        // In-place initialization
    }
}

impl<Format> FrameBuffer<Format, state::Initialized> {
    /// Freezes the buffer into a zero-copy thread-safe shared frame.
    pub fn freeze(self) -> FrameBuffer<Format, state::Shared> {
        // Converts unique storage to Arc-backed SharedFrameBuffer
    }
}
```

### 2.2 Zero-Cost Abstractions: Strided Sub-Views (`SubPixView<'a>`)
Instead of duplicating pixel buffers during cropping, `oxipix` provides zero-allocation strided views borrowing the underlying backing memory:

```rust
pub struct SubPixView<'a, Format> {
    data: &'a [u8],
    width: PixelWidth,
    height: PixelHeight,
    stride: PixelStride,
    _marker: std::marker::PhantomData<Format>,
}

impl<'a, Format: PixelType> SubPixView<'a, Format> {
    /// Returns a slice to an individual row in zero allocation.
    #[inline]
    pub fn row(&self, y: usize) -> Result<&'a [Format::Unit], PixError> {
        if y >= self.height.get() as usize {
            return Err(PixError::OutOfBounds { index: y, limit: self.height.get() as usize });
        }
        let start = y * self.stride.get();
        let end = start + (self.width.get() as usize * Format::BYTES_PER_PIXEL);
        // Safe checked slice borrowing without panics
        let slice = self.data.get(start..end).ok_or(PixError::BufferUnderflow)?;
        Ok(bytemuck::cast_slice(slice))
    }
}
```

### 2.3 RAII & Drop Semantics
Resource reclamation is strictly automatic:
- **`Drop` Implementation**: Any custom GPU buffers, memory-mapped handles, or scratch allocations cleanly release underlying resources upon drop.
- **Privacy Zeroize on Drop**: For sensitive document frames (e.g. KYC, medical, financial), `oxipix` provides `ZeroizeOnDropFrame` which zeroes memory lines using `zeroize::zeroize_mem` prior to releasing heap allocations.

### 2.4 Strongly Typed Newtypes
Stringly-typed or primitive-only interfaces are strictly prohibited. Geometric and codec parameters are wrapped in domain newtypes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PixelWidth(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PixelHeight(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PixelStride(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegQuality(u8);

impl JpegQuality {
    pub fn new(quality: u8) -> Result<Self, PixError> {
        if quality > 100 {
            return Err(PixError::InvalidQuality { quality });
        }
        Ok(Self(quality.clamp(1, 100)))
    }

    #[inline]
    pub fn get(self) -> u8 {
        self.0
    }
}
```

### 2.5 Error Handling Architecture
All library operations return strongly typed `Result<T, OxipixError>` leveraging `thiserror`. Production library code contains **zero `unwrap()`**, **zero `expect()`**, and **zero unbounded naked array indexing**.

```rust
#[derive(Debug, thiserror::Error)]
pub enum OxipixError {
    #[error("Dimension cannot be zero (width: {width}, height: {height})")]
    ZeroDimensions { width: u32, height: u32 },

    #[error("Buffer allocation size overflow ({width}x{height} * {bytes_per_pixel})")]
    AllocationOverflow { width: u32, height: u32, bytes_per_pixel: usize },

    #[error("Out of bounds access at index {index}, upper limit {limit}")]
    OutOfBounds { index: usize, limit: usize },

    #[error("Codec encoding failure: {reason}")]
    EncodingError { reason: String },

    #[error("Codec decoding failure: {reason}")]
    DecodingError { reason: String },

    #[error("Unsupported color format conversion from {from:?} to {to:?}")]
    UnsupportedColorConversion { from: String, to: String },

    #[error("Hardware SIMD dispatch error: {0}")]
    SimdDispatch(String),

    #[error("Hardware GPU accelerator error: {0}")]
    GpuAccelerator(String),
}
```

---

## 3. Anti-Patterns to Flag & Eliminate

During code generation and automated CI auditing, the following anti-patterns are strictly guarded against:

| Anti-Pattern | Violation in Existing Code | Strict `oxipix` Rule |
|:-------------|:---------------------------|:---------------------|
| **Excessive Cloning (`.clone()`)** | Calling `sub_image.to_image()` duplicates entire pixel buffers on every crop. | Use `SubPixView<'a>` for zero-copy borrows, or `Arc`-based `SharedFrame` with copy-on-write (`make_mut()`). |
| **God Structs & God Functions** | Single files handling decoders, encoders, filters, and pixel loops in > 800 LOC. | Modularity: max 800 lines per file; max 80 lines per function. Single Responsibility Principle. |
| **Hidden Panics (`unwrap()`, `[i]`)** | Naked slice indexing causes panic if image dimensions are malformed. | Mandatory boundary validation with `.get()` returning `PixError::OutOfBounds`. Zero panics. |
| **Stringly-Typed Interfaces** | Specifying formats as `"jpeg"`, `"png"` strings. | Strongly typed enum discriminators (`ImageFormat::Jpeg`, `PixelFormat::Rgba8`). |

---

## 4. Complexity Thresholds & Verification Metrics

In accordance with `/pattern-complexity-auditor`, all code in `oxipix` must adhere to the following metric thresholds:

1. **Cyclomatic Complexity (CC)**: $\le 10$ per function (hard limit $\le 15$).
2. **Cognitive Complexity**: $\le 15$ per function (hard limit $\le 20$).
3. **Lines of Code (LOC)**:
   - Functions: $\le 80$ lines.
   - Files: $\le 800$ lines.
4. **Clippy Linters Enforced in CI**:
   ```bash
   cargo clippy --all-targets --all-features -- \
     -D warnings \
     -W clippy::cognitive_complexity \
     -W clippy::too_many_lines \
     -W clippy::too_many_arguments \
     -W clippy::cast_possible_truncation
   ```
