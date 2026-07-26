//! `candle`-backed implementation of [`super::SentenceEncoder`] — see the
//! parent module's doc comment for the ort/candle/Ollama decision and the
//! RSS/binary-size and bundle-vs-download tradeoffs.

use super::SentenceEncoder;
use anyhow::{Context, Result};
use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig, DTYPE};
use std::path::Path;
use std::sync::Arc;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// The model this slice ships: a small, well-known, permissively-licensed
/// (Apache-2.0) sentence-transformer with documented `candle` BERT support.
/// See the parent module doc for why this exact model was chosen.
pub const DEFAULT_MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Files pulled from the HuggingFace repo. `model.safetensors` is the
/// weights file candle can mmap directly — deliberately not one of the
/// PyTorch/ONNX/OpenVINO/TF variants the repo also publishes.
const MODEL_FILES: &[&str] = &["config.json", "tokenizer.json", "model.safetensors"];

/// A local, CPU-only sentence encoder backed by `candle` + a BERT-family
/// model. Cheap to clone (the heavy state is behind `Arc`) and safe to share
/// across tasks — construct once via [`MiniLmEncoder::load`] (or, in
/// practice, via [`super::get_encoder`]'s process-global lazy loader) and
/// reuse.
pub struct MiniLmEncoder {
    model: Arc<BertModel>,
    tokenizer: Arc<Tokenizer>,
    model_id: String,
    dims: usize,
}

impl MiniLmEncoder {
    /// Loads the encoder, downloading model files into
    /// `models_dir/<model_id>/` first if they aren't already cached there.
    /// Network access is only needed on a cold cache.
    pub async fn load(models_dir: &Path, model_id: &str) -> Result<Self> {
        let model_dir = models_dir.join(model_id);
        download_if_missing(&model_dir, model_id).await?;

        let load_dir = model_dir.clone();
        let model_id_owned = model_id.to_string();
        // Reading + mmap'ing the weights and building the tensor graph is
        // blocking CPU/IO work — offload it to a blocking thread rather than
        // stalling the async runtime, mirroring this workspace's `run()`
        // pattern for blocking DB work (see root CLAUDE.md).
        tokio::task::spawn_blocking(move || Self::load_from_dir(&load_dir, &model_id_owned))
            .await
            .context("sentence-encoder model-load task panicked")?
    }

    fn load_from_dir(model_dir: &Path, model_id: &str) -> Result<Self> {
        let device = Device::Cpu;

        let config_path = model_dir.join("config.json");
        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        let config: BertConfig = serde_json::from_str(&config_str)
            .with_context(|| format!("parsing {}", config_path.display()))?;
        let dims = config.hidden_size;

        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("loading tokenizer {}: {e}", tokenizer_path.display()))?;

        let weights_path = model_dir.join("model.safetensors");
        // SAFETY: mmap'ing a safetensors file this process downloaded itself
        // (or a previous run did) into `<data_dir>/models/...` — not
        // attacker-controlled input reachable from a request.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(std::slice::from_ref(&weights_path), DTYPE, &device)
        }
        .with_context(|| format!("mmap'ing weights {}", weights_path.display()))?;
        let model = BertModel::load(vb, &config).context("constructing BertModel")?;

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            model_id: model_id.to_string(),
            dims,
        })
    }
}

#[async_trait]
impl SentenceEncoder for MiniLmEncoder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dims(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let model = Arc::clone(&self.model);
        let tokenizer = Arc::clone(&self.tokenizer);
        let texts = texts.to_vec();
        // Forward pass is CPU-bound; keep it off the async runtime like the
        // load path above.
        tokio::task::spawn_blocking(move || embed_blocking(&model, &tokenizer, &texts))
            .await
            .context("sentence-encoder embed task panicked")?
    }
}

fn embed_blocking(
    model: &BertModel,
    tokenizer: &Tokenizer,
    texts: &[String],
) -> Result<Vec<Vec<f32>>> {
    let device = Device::Cpu;
    let mut tokenizer = tokenizer.clone();
    tokenizer
        .with_truncation(Some(TruncationParams {
            max_length: 512,
            ..Default::default()
        }))
        .map_err(|e| anyhow::anyhow!("configuring truncation: {e}"))?;
    // Only pad when batching more than one text — a lone text needs no
    // padding, and skipping it keeps the single-text path byte-identical to
    // what a batch-of-one would produce with the real (non-padded) length.
    let padding = if texts.len() > 1 {
        Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        })
    } else {
        None
    };
    tokenizer.with_padding(padding);

    let encodings = tokenizer
        .encode_batch(texts.to_vec(), true)
        .map_err(|e| anyhow::anyhow!("tokenizing: {e}"))?;

    let token_ids = encodings
        .iter()
        .map(|enc| Ok(Tensor::new(enc.get_ids(), &device)?))
        .collect::<Result<Vec<Tensor>>>()?;
    let attention_mask = encodings
        .iter()
        .map(|enc| Ok(Tensor::new(enc.get_attention_mask(), &device)?))
        .collect::<Result<Vec<Tensor>>>()?;

    let token_ids = Tensor::stack(&token_ids, 0)?;
    let attention_mask = Tensor::stack(&attention_mask, 0)?;
    let token_type_ids = token_ids.zeros_like()?;

    let output = model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;

    // Mean-pool over the sequence dimension, excluding padded positions via
    // the attention mask. Masking out padding here is what keeps batched
    // output numerically close to embedding each text alone: padded
    // positions never contribute to the mean regardless of how long the
    // batch's longest text made the padded sequence dimension.
    let mask = attention_mask.to_dtype(DTYPE)?.unsqueeze(2)?;
    let summed = output.broadcast_mul(&mask)?.sum(1)?;
    let counts = mask.sum(1)?;
    let pooled = summed.broadcast_div(&counts)?;
    let normalized = normalize_l2(&pooled)?;

    let mut out = Vec::with_capacity(texts.len());
    for i in 0..texts.len() {
        out.push(normalized.get(i)?.to_vec1::<f32>()?);
    }
    Ok(out)
}

fn normalize_l2(v: &Tensor) -> candle_core::Result<Tensor> {
    v.broadcast_div(&v.sqr()?.sum_keepdim(1)?.sqrt()?)
}

/// Downloads any of [`MODEL_FILES`] not already present in `model_dir` from
/// the public HuggingFace resolve endpoint. A file already on disk is
/// assumed complete and is not re-fetched — downloads write to a `.part`
/// sibling and rename over the real name only on success, so a crash or
/// killed process mid-download never leaves a truncated file that a later
/// run would mistake for a cached one.
async fn download_if_missing(model_dir: &Path, model_id: &str) -> Result<()> {
    if MODEL_FILES.iter().all(|f| model_dir.join(f).is_file()) {
        return Ok(());
    }
    std::fs::create_dir_all(model_dir)
        .with_context(|| format!("creating {}", model_dir.display()))?;

    let client = reqwest::Client::new();
    for file in MODEL_FILES {
        let dest = model_dir.join(file);
        if dest.is_file() {
            continue;
        }
        let url = format!("https://huggingface.co/{model_id}/resolve/main/{file}");
        tracing::info!(url = %url, "downloading sentence-encoder model file (first use only)");
        let resp = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("requesting {url}"))?
            .error_for_status()
            .with_context(|| format!("fetching {url}"))?;
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("reading response body for {url}"))?;

        let tmp = dest.with_extension("part");
        std::fs::write(&tmp, &bytes).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &dest)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), dest.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cosine similarity between two equal-length vectors. Panics (via
    /// `assert`s in callers) rather than returning `Result` — this is test
    /// support code, not production surface.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (norm_a * norm_b)
    }

    /// Reuse a fixed cache dir across test runs (rather than a fresh
    /// tempdir) so repeated local/CI runs of this ignored test don't
    /// re-download ~90MB from HuggingFace every time — exactly the
    /// present-file cache behavior `download_if_missing` is meant to give
    /// real deployments too.
    fn test_models_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("finsight-sentence-encoder-test-cache")
    }

    /// End-to-end: downloads (or reuses a cached) real MiniLM model and
    /// exercises the full `embed()` path. Requires network access on a cold
    /// cache. Gated `#[ignore]` like this repo's other live-network tests
    /// (see e.g. `crates/finsight-bindings/tests/copilot_live.rs`), so
    /// `cargo test --workspace` stays hermetic; run explicitly with
    /// `cargo test -p finsight-agent --lib embedding::candle_encoder::tests -- --ignored`.
    #[tokio::test]
    #[ignore = "downloads ~90MB of real model weights from HuggingFace on a cold cache; network required"]
    async fn embed_end_to_end_real_model() {
        let encoder = MiniLmEncoder::load(&test_models_dir(), DEFAULT_MODEL_ID)
            .await
            .expect("model should load (downloading on a cold cache)");

        // (3) dims() matches the actual returned vector length.
        assert_eq!(encoder.dims(), 384, "MiniLM-L6-v2 is a 384-dim model");

        let texts = vec![
            "I love hiking in the mountains on weekends".to_string(),
            "Mountain trails are my favorite weekend activity".to_string(),
            "The stock market closed lower today amid inflation fears".to_string(),
        ];
        let embeddings = encoder
            .embed(&texts)
            .await
            .expect("batch embed should succeed");
        assert_eq!(embeddings.len(), texts.len());
        for v in &embeddings {
            assert_eq!(v.len(), encoder.dims());
        }

        // (1) Two semantically similar sentences (both about mountain
        // hiking) must be more cosine-similar to each other than either is
        // to an unrelated sentence (stock market news) — a real correctness
        // check, not just "it doesn't crash".
        let sim_similar = cosine_similarity(&embeddings[0], &embeddings[1]);
        let sim_dissimilar_a = cosine_similarity(&embeddings[0], &embeddings[2]);
        let sim_dissimilar_b = cosine_similarity(&embeddings[1], &embeddings[2]);
        println!("cosine(similar pair)      = {sim_similar}");
        println!("cosine(dissimilar pair a) = {sim_dissimilar_a}");
        println!("cosine(dissimilar pair b) = {sim_dissimilar_b}");
        assert!(
            sim_similar > sim_dissimilar_a && sim_similar > sim_dissimilar_b,
            "similar-topic sentences should cosine-score higher than unrelated ones: \
             similar={sim_similar}, dissimilar_a={sim_dissimilar_a}, dissimilar_b={sim_dissimilar_b}"
        );

        // (2) Batching N texts in one call should match embedding each text
        // separately, within a small numerical tolerance (padding to the
        // batch's longest sequence changes matmul shapes, so bit-identical
        // output isn't guaranteed — the attention-mask-aware mean pooling
        // above is what keeps them close).
        let mut separate = Vec::with_capacity(texts.len());
        for t in &texts {
            let one = encoder
                .embed(std::slice::from_ref(t))
                .await
                .expect("single embed should succeed");
            separate.push(one.into_iter().next().unwrap());
        }
        for (i, (batched, alone)) in embeddings.iter().zip(separate.iter()).enumerate() {
            let sim = cosine_similarity(batched, alone);
            println!("cosine(batched vs alone) for text[{i}] = {sim}");
            assert!(
                sim > 0.999,
                "batched and single-call embeddings for the same text should \
                 be numerically close: text[{i}] cosine={sim}"
            );
        }
    }
}
