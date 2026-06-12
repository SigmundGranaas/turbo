//! Generates the Kotlin/Swift binding sources from the compiled cdylib:
//!
//! ```sh
//! cargo build -p turbomap-ffi
//! cargo run -p turbomap-ffi --bin uniffi-bindgen -- \
//!   generate --library target/debug/libturbomap_ffi.so \
//!   --language kotlin --language swift --out-dir target/ffi-bindings
//! ```

fn main() {
    uniffi::uniffi_bindgen_main()
}
