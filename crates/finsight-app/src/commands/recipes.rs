use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_agent::{context, planner};
use finsight_core::models::{AgentRecipe, AgentRecipeRun};
use finsight_core::repos::{recipes, run};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_recipes(
    state: State<'_, AppState>,
    include_paused: bool,
) -> AppResult<Vec<AgentRecipe>> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::list(conn, include_paused))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_recipe(
    state: State<'_, AppState>,
    title: String,
    description: String,
    recipe_kind: String,
    prompt_template: String,
    cadence: String,
    day_of_week: Option<i64>,
    day_of_month: Option<i64>,
) -> AppResult<AgentRecipe> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        recipes::insert(
            conn,
            &title,
            &description,
            &recipe_kind,
            &prompt_template,
            &cadence,
            day_of_week,
            day_of_month,
        )
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_recipe(
    state: State<'_, AppState>,
    id: String,
    title: String,
    description: String,
    prompt_template: String,
    cadence: String,
    day_of_week: Option<i64>,
    day_of_month: Option<i64>,
) -> AppResult<AgentRecipe> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        recipes::update(
            conn,
            &id,
            &title,
            &description,
            &prompt_template,
            &cadence,
            day_of_week,
            day_of_month,
        )
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn pause_recipe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::set_status(conn, &id, "paused"))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn resume_recipe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::set_status(conn, &id, "active"))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_recipe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::set_status(conn, &id, "deleted"))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn trigger_recipe(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let db = (*state.db).clone();
    let recipe_id_for_load = id.clone();
    let recipe = run(&db, move |conn| recipes::get(conn, &recipe_id_for_load))
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new("recipe.not_found", format!("Recipe '{id}' was not found."))
        })?;

    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        return Err(AppError::new(
            "no_provider",
            "Configure an AI provider in Settings → Agent before running recipes.",
        ));
    };

    // Snapshot the ledger epoch before build_context + the LLM call so we can
    // refuse to persist the bundle if a Delete-All lands during the run.
    let start_epoch = db.reset_barrier().epoch();

    let recipe_id_for_run = recipe.id.clone();
    let recipe_run = run(&db, move |conn| {
        recipes::start_run(conn, &recipe_id_for_run)
    })
    .await
    .map_err(AppError::from)?;

    let ctx = match run(&db, |conn| Ok(context::build_context(conn))).await {
        Ok(ctx) => ctx,
        Err(err) => {
            let run_id = recipe_run.id.clone();
            let message = err.to_string();
            let _ = run(&db, move |conn| recipes::fail_run(conn, &run_id, &message)).await;
            return Err(AppError::from(err));
        }
    };

    let prompt = format!("[Recipe: {}] {}", recipe.title, recipe.prompt_template);
    let llm_json = match provider
        .complete_json(&planner::build_system_prompt(&ctx), &prompt)
        .await
    {
        Ok(json) => json,
        Err(err) => {
            let run_id = recipe_run.id.clone();
            let message = err.to_string();
            let _ = run(&db, move |conn| recipes::fail_run(conn, &run_id, &message)).await;
            return Err(AppError::new("recipe.llm", err.to_string()));
        }
    };

    // Hold a reset lease across the bundle commit; skip if a Delete-All landed
    // during the LLM call so no proposed bundle survives the wipe.
    let plan_lease = db.reset_barrier().writer_lease(start_epoch).await;
    if plan_lease.superseded() {
        let run_id = recipe_run.id.clone();
        let _ = run(&db, move |conn| {
            recipes::fail_run(conn, &run_id, "cancelled: data was cleared during the run")
        })
        .await;
        return Err(AppError::new(
            "reset",
            "Recipe cancelled: all data was cleared during the run.",
        ));
    }

    let run_id = recipe_run.id.clone();
    let prompt_for_persist = prompt.clone();
    let provider_id = provider.provider_id().to_string();
    let model_id = provider.model_id().to_string();
    match run(&db, move |conn| {
        let result = planner::persist_plan(
            conn,
            None,
            &prompt_for_persist,
            &llm_json,
            &provider_id,
            &model_id,
        )?;
        let bundle_id = result.bundle.id.clone();
        recipes::complete_run(conn, &run_id, &bundle_id)?;
        Ok(bundle_id)
    })
    .await
    {
        Ok(bundle_id) => Ok(bundle_id),
        Err(err) => {
            let run_id = recipe_run.id.clone();
            let message = err.to_string();
            let _ = run(&db, move |conn| recipes::fail_run(conn, &run_id, &message)).await;
            Err(AppError::from(err))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn list_recipe_runs(
    state: State<'_, AppState>,
    recipe_id: String,
    limit: Option<u32>,
) -> AppResult<Vec<AgentRecipeRun>> {
    let db = (*state.db).clone();
    let limit = limit.unwrap_or(10);
    run(&db, move |conn| recipes::list_runs(conn, &recipe_id, limit))
        .await
        .map_err(AppError::from)
}
