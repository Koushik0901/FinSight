//! Local CPU sentence-embedding service (issue #90, part of the
//! categorization epic #74). This module ships the encoder capability only —
//! semantic category matching against it is issue #92, not this slice.
//!
//! # Decision: `candle` over `ort` / Ollama
//!
//! Epic #74's title pre-decided "quantized ONNX". This slice resolves that
//! fork explicitly instead of inheriting it:
//!
//! - **`ort` (ONNX Runtime bindings)** — pulls in a native C++ runtime. This
//!   repo already works around a fragile vendored-OpenSSL + Strawberry-Perl
//!   toolchain (see root `CLAUDE.md` → "Cargo/Perl toolchain"); a second
//!   native dependency compounds that fragility for every contributor and CI
//!   runner, not just the ones exercising semantic categorization.
//! - **External Ollama `/api/embeddings`** — `LlmProviderConfig::Ollama`
//!   already carries a half-scaffolded `embedding_model` field (see
//!   `crates/finsight-api/src/commands/onboarding.rs`), but routing through
//!   it would force every self-hosted deployment to also run and reach a
//!   separate Ollama process. This repo's self-hosting pitch
//!   (`docs/self-hosting.md`) is "one container, no extra services" — an
//!   external dependency for a categorization nicety cuts against that.
//! - **`candle` (pure Rust) — chosen.** Its own tensor/model code has no
//!   native linking (unlike `ort`'s full onnxruntime), and it runs entirely
//!   on CPU, matching this repo's actual deploy targets: a thin Tauri webview
//!   shell with no local compute budget to spare, and a modest self-hosted
//!   NAS/Docker box — neither is a GPU box. Not *zero* native surface area
//!   though — see the binary-size note below, `tokenizers` still brings in
//!   one small native shim candle doesn't control.
//!
//! ## RSS / binary-size tradeoff (explicit, per the issue)
//!
//! Whichever of the three paths were picked, self-hosters pay *something* on
//! every deploy, whether or not semantic categorization is ever invoked:
//!
//! - **Binary size**: `candle-core` + `candle-nn` + `candle-transformers` +
//!   `tokenizers` add roughly tens of MB to the compiled
//!   `finsight-agent`-linked artifacts (a CPU-only build — no `cuda`/`mkl`/
//!   `accelerate` feature is enabled). This is not a fully native-linking-free
//!   build, though: `tokenizers`' default features pull in `onig`/`onig_sys`
//!   (native C, Oniguruma regex). Our own direct `tokenizers` dependency (see
//!   `Cargo.toml`) drops it: `onig` is swapped for `fancy-regex`, a pure-Rust
//!   regex engine (`tokenizers` requires exactly one of the two to compile at
//!   all — neither is truly optional). `candle-core` 0.11 still pins its own
//!   separate `tokenizers 0.22` line with its `onig` feature on — a different
//!   semver-major (0.x) line, so Cargo can't unify features across it — so
//!   `onig`/`onig_sys` still enters the build graph transitively via
//!   `candle-core` regardless, not fixable here without patching/forking it
//!   (confirmed via `cargo tree -p finsight-agent -i onig_sys`; `esaxx-rs`,
//!   `tokenizers`' other potentially-native dependency, resolves with zero
//!   features enabled anywhere in this graph per `cargo metadata`, so its
//!   native C++ `cpp` acceleration path is not active here — it builds pure
//!   Rust). `candle` is still the meaningfully smaller and less fragile
//!   choice: one small, single-purpose native regex shim, not a full
//!   onnxruntime C++ runtime (`ort`'s alternative, see the decision above)
//!   plus its own bundled shared library per target platform.
//! - **RSS**: model *weights* (~90MB on disk for MiniLM, see below) are not
//!   loaded at process start — see "Lazy load" below — so a server that
//!   never triggers semantic categorization pays no extra idle RSS. Once
//!   loaded (first call), expect the encoder to hold onto roughly
//!   100-200MB RSS for the remaining life of the process (mmapped
//!   safetensors + candle's CPU tensor buffers + tokenizer vocab). This
//!   slice has no unload path — acceptable for a long-lived server process,
//!   worth knowing on a memory-constrained NAS.
//!
//! ## Bundle vs. download (explicit)
//!
//! Model weights are **not** committed to git or bundled into the compiled
//! binary. [`candle_encoder`] downloads them on first real use from
//! HuggingFace, pinned to a specific commit — never the moving `main` ref
//! (`https://huggingface.co/<model_id>/resolve/<revision>/<file>`, public
//! repo, no auth needed) — into
//! `<FINSIGHT_DATA_DIR>/models/<model_id>/<revision>/`, the same
//! `FINSIGHT_DATA_DIR` that already holds the per-user SQLCipher DBs, so
//! model weights live alongside other self-hosted state instead of a new
//! top-level convention. Putting the revision in the cache path (not just
//! the URL) matters: it means a future revision bump can never be mistaken
//! for already-cached weights by the present-file check — old and new
//! revisions simply live at different paths. Downloads reuse the workspace's
//! existing `reqwest` dependency — no second HTTP client.
//!
//! **Known limitation, intentionally not solved here**: a deployment with no
//! internet egress (e.g. an air-gapped NAS behind a locked-down firewall)
//! cannot fetch the ~90MB of weights on first use, and semantic
//! categorization will fail with a download error until the operator
//! pre-populates `<data_dir>/models/<model_id>/<revision>/` by hand.
//! Air-gapped deployment support is out of scope for this slice.
//!
//! ## Model
//!
//! [`candle_encoder::DEFAULT_MODEL_ID`] —
//! `sentence-transformers/all-MiniLM-L6-v2`: 384 dims, ~90MB in
//! `safetensors`, Apache-2.0 licensed (compatible with this project's
//! AGPL-3.0-or-later license). Chosen because it's the canonical small
//! sentence-embedding model with first-class, documented `candle` BERT
//! support (`candle_transformers::models::bert`), so no custom model porting
//! is needed.
//!
//! ## Lazy load
//!
//! [`get_encoder`] loads weights into memory only on the first call it
//! serves (guarded by a `tokio::sync::OnceCell`), never at process start —
//! concurrent callers all await the same in-flight load rather than each
//! triggering their own.
//!
//! ## Deferred: "measured against #88's harness"
//!
//! The epic's acceptance line asking for precision to be measured against
//! issue #88's eval harness cannot be fulfilled by this slice: it requires
//! issue #92 (semantic proposal matching, not yet built) to exist first, as
//! something to measure. Explicitly deferred to #92, not skipped or
//! forgotten.

pub mod candle_encoder;

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::OnceCell;

/// A versioned local sentence-embedding backend.
///
/// `model_id` is load-bearing for callers beyond this crate: issue #92 will
/// tag stored embedding vectors with it, so a future model swap can detect
/// and invalidate stale vectors instead of silently comparing embeddings
/// from two different vector spaces.
#[async_trait]
pub trait SentenceEncoder: Send + Sync {
    /// Stable identifier for the loaded model/revision (e.g.
    /// `"sentence-transformers/all-MiniLM-L6-v2"`). Callers that persist
    /// embeddings should store this alongside the vector.
    fn model_id(&self) -> &str;

    /// Length of every vector [`Self::embed`] returns.
    fn dims(&self) -> usize;

    /// Embeds a batch of texts, one output vector per input, same order,
    /// each of length [`Self::dims`]. Batching multiple texts in one call is
    /// more efficient than N separate calls where the backend allows shared
    /// padding + a single forward pass (as the `candle` implementation
    /// does) — prefer batching over looping when embedding many texts.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

static ENCODER: OnceCell<Arc<dyn SentenceEncoder>> = OnceCell::const_new();

/// Returns the process-global encoder, loading it — and downloading model
/// weights into `<data_dir>/models/...` if not already cached — only on the
/// first call. Every later call, from any task, reuses the already-loaded
/// model; concurrent first callers all await the same in-flight load rather
/// than racing to load it independently (`OnceCell` semantics).
///
/// `data_dir` is only consulted on the first call (it is the process's one
/// `FINSIGHT_DATA_DIR`, so later calls would only ever see the same value
/// anyway).
pub async fn get_encoder(data_dir: &Path) -> Result<Arc<dyn SentenceEncoder>> {
    let models_dir = data_dir.join("models");
    ENCODER
        .get_or_try_init(|| async {
            let encoder =
                candle_encoder::MiniLmEncoder::load(&models_dir, candle_encoder::DEFAULT_MODEL_ID)
                    .await?;
            Ok::<Arc<dyn SentenceEncoder>, anyhow::Error>(Arc::new(encoder))
        })
        .await
        .map(Arc::clone)
}
