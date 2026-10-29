//! Vietnamese Telex input method engine.
//!
//! ```
//! assert_eq!(telex::convert("Tooi yeeu Vieetj Nam"), "Tôi yêu Việt Nam");
//! ```

pub mod engine;
pub mod syllable;
pub mod unicode;

#[cfg(test)]
mod tests;

pub use engine::{TelexEngine, convert};
