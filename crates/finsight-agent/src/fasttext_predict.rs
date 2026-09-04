//! Local fastText merchant classifier (feature `fasttext-local`).
//!
//! Mirrors `poc/fasttext_categorizer/normalize.py::merchant_for_training`
//! and `finsight_core::merchant::normalize_merchant` so training and
//! inference stay in sync. Model is the 39 MB `merchant_ft.bin` trained on
//! 1212 merchants (POC 96% acc, see docs/llm-replacement-audit.md).
//!
//! Loading is lazy + process-global like `embedding::get_encoder`:
//! weights live at `<FINSIGHT_DATA_DIR>/models/fasttext/merchant_ft.bin` with
//! dev fallback to `poc/fasttext_categorizer/models/merchant_ft.bin`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tokio::sync::OnceCell;

#[cfg(feature = "fasttext-local")]
use fasttext::FastText;

const MODEL_FILE: &str = "merchant_ft.bin";
const MODEL_SUBDIR: &str = "models/fasttext";

fn amount_bucket(amount_cents: i64) -> &'static str {
    if amount_cents > 0 {
        "income"
    } else if amount_cents >= -2000 {
        "small"
    } else if amount_cents >= -10000 {
        "medium"
    } else {
        "large"
    }
}

fn redact_for_model(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut digit_run = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            digit_run.push(ch);
        } else {
            if !digit_run.is_empty() {
                if digit_run.len() >= 4 {
                    out.push('#');
                } else {
                    out.push_str(&digit_run);
                }
                digit_run.clear();
            }
            out.push(ch);
        }
    }
    if !digit_run.is_empty() {
        if digit_run.len() >= 4 {
            out.push('#');
        } else {
            out.push_str(&digit_run);
        }
    }
    out
}

pub fn merchant_text_for_model(merchant_raw: &str, amount_cents: i64) -> String {
    let redacted = redact_for_model(merchant_raw);
    let base = finsight_core::merchant::normalize_merchant(&redacted);
    let base = if base.is_empty() {
        redacted
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        base
    };
    format!("{} __amount_{}", base, amount_bucket(amount_cents))
}

#[cfg(feature = "fasttext-local")]
pub struct FastTextHandle {
    inner: Mutex<FastText>,
}

#[cfg(feature = "fasttext-local")]
static MODEL: OnceCell<Arc<FastTextHandle>> = OnceCell::const_new();

#[cfg(feature = "fasttext-local")]
fn model_paths(data_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(data_dir.join(MODEL_SUBDIR).join(MODEL_FILE));
    if let Ok(env_dir) = std::env::var("FINSIGHT_DATA_DIR") {
        candidates.push(PathBuf::from(&env_dir).join(MODEL_SUBDIR).join(MODEL_FILE));
        candidates.push(PathBuf::from(&env_dir).join(MODEL_FILE));
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(
            Path::new(&manifest)
                .join("../../poc/fasttext_categorizer/models")
                .join(MODEL_FILE),
        );
    }
    candidates.push(PathBuf::from("poc/fasttext_categorizer/models").join(MODEL_FILE));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(MODEL_FILE));
        }
    }
    candidates
}

#[cfg(feature = "fasttext-local")]
pub async fn get_fasttext_model(data_dir: &Path) -> Result<Arc<FastTextHandle>> {
    let dir = data_dir.to_path_buf();
    MODEL
        .get_or_try_init(move || async move {
            let mut last_err = None;
            for path in model_paths(&dir) {
                if path.exists() {
                    let path_clone = path.clone();
                    let handle = tokio::task::spawn_blocking(move || {
                        let mut ft = FastText::new();
                        let p = path_clone.to_string_lossy().to_string();
                        ft.load_model(&p).map_err(|e| {
                            anyhow::anyhow!("load fasttext model {}: {}", path_clone.display(), e)
                        })?;
                        Ok::<_, anyhow::Error>(FastTextHandle {
                            inner: Mutex::new(ft),
                        })
                    })
                    .await
                    .context("fasttext load join")??;
                    tracing::info!("[fasttext] loaded model {}", path.display());
                    return Ok(Arc::new(handle));
                } else {
                    last_err = Some(path.display().to_string());
                }
            }
            anyhow::bail!(
                "fasttext model not found; tried {:?}, last missing: {:?}",
                model_paths(&dir),
                last_err
            );
        })
        .await
        .cloned()
}

#[cfg(feature = "fasttext-local")]
impl FastTextHandle {
    pub fn predict(&self, text: &str) -> Option<(String, f64)> {
        let guard = self.inner.lock().ok()?;
        let preds = guard.predict(text, 1, 0.0).ok()?;
        if preds.is_empty() {
            return None;
        }
        let first = &preds[0];
        let label = first.label.trim_start_matches("__label__").to_string();
        let prob = first.prob as f64;
        Some((label, prob))
    }
}

#[cfg(not(feature = "fasttext-local"))]
pub async fn get_fasttext_model(_data_dir: &Path) -> Result<Arc<()>> {
    anyhow::bail!("fasttext-local feature not enabled")
}
