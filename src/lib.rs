#![deny(
    missing_docs,
    missing_debug_implementations,
    dead_code,
    clippy::missing_docs_in_private_items
)]
#![warn(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::print_stdout,
    clippy::cast_sign_loss
)]
//! Safe Libass bindings for Rust
//!

pub mod library;
pub mod track;
