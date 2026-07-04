use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::run;
use finsight_core::settings;
use serde::Serialize;
use specta::Type;

const KEY_COMPLETION: &str = "onboarding_completion_marked";

#[derive(Debug, Clone, Serialize, Type)]
pub struct OnboardingState {
    pub account_count: i64,
    pub category_count: i64,
    pub completion_marked: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_state(state: tauri::State<'_, AppState>) -> AppResult<OnboardingState> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let account_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM accounts WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let category_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let completion_marked: bool = settings::get::<bool>(conn, KEY_COMPLETION)?.unwrap_or(false);
        Ok(OnboardingState {
            account_count,
            category_count,
            completion_marked,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_onboarding_complete(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| settings::set(conn, KEY_COMPLETION, &true))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_onboarding_completion(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| settings::set(conn, KEY_COMPLETION, &false))
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct OllamaProbeResult {
    pub reachable: bool,
    pub models: Vec<String>,
    pub has_nomic_embed: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn probe_ollama(base_url: String) -> AppResult<OllamaProbeResult> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| AppError::new("http", e.to_string()))?;
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(OllamaProbeResult {
                reachable: false,
                models: vec![],
                has_nomic_embed: false,
            })
        }
    };
    if !resp.status().is_success() {
        return Ok(OllamaProbeResult {
            reachable: false,
            models: vec![],
            has_nomic_embed: false,
        });
    }
    #[derive(serde::Deserialize)]
    struct TagsResp {
        models: Vec<Tag>,
    }
    #[derive(serde::Deserialize)]
    struct Tag {
        name: String,
    }
    let body: TagsResp = match resp.json().await {
        Ok(b) => b,
        Err(_) => {
            return Ok(OllamaProbeResult {
                reachable: false,
                models: vec![],
                has_nomic_embed: false,
            })
        }
    };
    let models: Vec<String> = body.models.into_iter().map(|m| m.name).collect();
    let has_nomic_embed = models.iter().any(|m| m.starts_with("nomic-embed-text"));
    Ok(OllamaProbeResult {
        reachable: true,
        models,
        has_nomic_embed,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind")]
pub enum LlmProviderConfig {
    #[serde(rename = "ollama")]
    Ollama {
        base_url: String,
        completion_model: String,
        embedding_model: String,
    },
    #[serde(rename = "unconfigured")]
    Unconfigured,
}

#[tauri::command]
#[specta::specta]
pub async fn save_llm_provider(
    state: tauri::State<'_, AppState>,
    config: LlmProviderConfig,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, "llm_provider", &config)
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct StarterCategory {
    pub id: String,
    pub label: String,
    pub group_id: String,
    pub color: String,
}

#[tauri::command]
#[specta::specta]
pub async fn commit_starter_categories(
    state: tauri::State<'_, AppState>,
    categories: Vec<StarterCategory>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let tx = conn.transaction()?;
        for (gid, label) in [
            ("fixed", "Fixed"),
            ("daily", "Daily"),
            ("lifestyle", "Lifestyle"),
            ("wellbeing", "Wellbeing"),
        ] {
            tx.execute(
                "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
                rusqlite::params![gid, label],
            )?;
        }
        for c in &categories {
            // Known starter ids get their conscious-spending bucket up front so
            // the Budget "Spending mix" works out of the box; custom categories
            // stay untagged until the user decides.
            let spending_type = finsight_core::categorize::default_spending_type(&c.id);
            tx.execute(
                "INSERT OR IGNORE INTO categories(id, group_id, label, color, spending_type, sort_order) \
                 VALUES(?1, ?2, ?3, ?4, ?5, 0)",
                rusqlite::params![c.id, c.group_id, c.label, c.color, spending_type],
            )?;
        }
        tx.commit()?;
        // Onboarding imports transactions (step 2) before categories are
        // committed (step 3), so anything imported earlier has no categories to
        // match against. Now that the starter categories exist, run the
        // deterministic categorizer so those transactions get a stable category.
        // Idempotent and best-effort — never block category setup on it.
        let _ = finsight_core::categorize::apply_builtin_categorization(conn);
        Ok(())
    })
    .await
    .map_err(AppError::from)
}
