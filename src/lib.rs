//! # docula
//!
//! Sovereign, pure-Rust zero-FFI document intelligence engine.

pub use docula_core::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_reexports() {
        let point = Point::new(0.1, 0.2).expect("valid normalized point");
        assert_eq!(point.x, 0.1);
        assert_eq!(point.y, 0.2);
    }
}
