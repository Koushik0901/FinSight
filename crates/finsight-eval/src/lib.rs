//! Library surface for `finsight-eval`.
//!
//! The Copilot answer-quality benchmark (`main.rs`) is a standalone bin and
//! doesn't need this. This lib exists so the categorization precision/coverage
//! harness (issue #88, "Slice 2") and its labeled-corpus format (issue #89,
//! "Slice 2b") are:
//!   - testable via `cargo test -p finsight-eval` (unit tests live next to the
//!     code in each module, matching the rest of this crate's convention —
//!     see `src/seed.rs`), and
//!   - shared between the `categorization_eval` bin (`src/bin/`) and any
//!     future consumer without duplicating logic.
//!
//! See `crates::categorization` for the harness itself and
//! `eval/CATEGORIZATION_CORPUS.md` for the corpus format + methodology.

pub mod categorization;
