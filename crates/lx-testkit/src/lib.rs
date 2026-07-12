#![forbid(unsafe_code)]

//! Test helpers for LX Coreutils.
//!
//! This crate is a `dev-dependency` only — it is never compiled into
//! production binaries. Every tool's integration tests import from here.
//!
//! # Modules
//! - [`mock`] — `MockLlmClient` with fixed responses and call capture
//! - [`recording`] — `RecordingLlmClient` wrapping a real client for eval tests
//! - [`assertions`] — shared assertion helpers used in every tool's test suite

pub mod assertions;
pub mod binary;
pub mod mock;
pub mod recording;

// Flat re-exports for ergonomic use in tool tests.
pub use assertions::{
    assert_image_in_request, assert_lang_placeholder_in_system, assert_no_secrets_in_request,
    assert_request_invariants,
};
pub use mock::{CapturedRequest, MockLlmClient};
pub use recording::RecordingLlmClient;
