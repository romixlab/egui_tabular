//! Headless UI tests for `TableView`, driven through egui_kittest (AccessKit + synthetic input).
//!
//! Tests that describe the intended behavior of a known bug are `#[ignore = "<BUG-ID>: ..."]`.
//! Run them with `cargo test --test ui -- --ignored`; when one starts passing, the bug is fixed:
//! remove the `ignore` and move the bug to "Fixed issues" in FEATURES.md.

mod columns;
mod editing;
mod fixture;
mod keyboard;
mod paste;
mod selection;
