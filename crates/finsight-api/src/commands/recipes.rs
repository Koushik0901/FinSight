use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_agent::{context, planner};
use finsight_core::models::{AgentRecipe, AgentRecipeRun};
use finsight_core::repos::{recipes, run};

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ListRecipesRequest {
    pub include_paused: bool,
}

#[utoipa::path(post, path = "/api/rpc/list_recipes",
    request_body(content = ListRecipesRequest), responses((status = 200, body = Vec<AgentRecipe>)))]
pub async fn list_recipes(state: &ApiState, include_paused: bool) -> AppResult<Vec<AgentRecipe>> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::list(conn, include_paused))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreateRecipeRequest {
    pub title: String,
    pub description: String,
    pub recipe_kind: String,
    pub prompt_template: String,
    pub cadence: String,
    pub day_of_week: Option<i64>,
    pub day_of_month: Option<i64>,
}

#[utoipa::path(post, path = "/api/rpc/create_recipe", request_body(content = CreateRecipeRequest), responses((status = 200, body = AgentRecipe)))]
pub async fn create_recipe(
    state: &ApiState,
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

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct UpdateRecipeRequest {
    pub id: String,
    pub title: String,
    pub description: String,
    pub prompt_template: String,
    pub cadence: String,
    pub day_of_week: Option<i64>,
    pub day_of_month: Option<i64>,
}

#[utoipa::path(post, path = "/api/rpc/update_recipe", request_body(content = UpdateRecipeRequest), responses((status = 200, body = AgentRecipe)))]
pub async fn update_recipe(
    state: &ApiState,
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

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct PauseRecipeRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/pause_recipe",
    request_body(content = PauseRecipeRequest), responses((status = 200, description = "Success")))]
pub async fn pause_recipe(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::set_status(conn, &id, "paused"))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ResumeRecipeRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/resume_recipe",
    request_body(content = ResumeRecipeRequest), responses((status = 200, description = "Success")))]
pub async fn resume_recipe(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::set_status(conn, &id, "active"))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DeleteRecipeRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/delete_recipe",
    request_body(content = DeleteRecipeRequest), responses((status = 200, description = "Success")))]
pub async fn delete_recipe(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| recipes::set_status(conn, &id, "deleted"))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct TriggerRecipeRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/trigger_recipe",
    request_body(content = TriggerRecipeRequest), responses((status = 200, content_type = "application/json", body = String)))]
pub async fn trigger_recipe(state: &ApiState, id: String) -> AppResult<String> {
    let db = (*state.db).clone();
    let recipe_id_for_load = id.clone();
    let recipe = run(&db, move |conn| recipes::get(conn, &recipe_id_for_load))
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new("recipe.not_found", format!("Recipe '{id}' was not found."))
        })?;

    let provider_opt = state.agent_provider.read().unwrap().clone();

    // Snapshot the ledger epoch before build_context + the LLM call so we can
    // refuse to persist the bundle if a Delete-All lands during the run.
    let start_epoch = db.reset_barrier().epoch();

    let recipe_id_for_run = recipe.id.clone();
    let recipe_run = run(&db, move |conn| {
        recipes::start_run(conn, &recipe_id_for_run)
    })
    .await
    .map_err(AppError::from)?;

    let ctx = match run(&db, context::build_context).await {
        Ok(ctx) => ctx,
        Err(err) => {
            let run_id = recipe_run.id.clone();
            let message = err.to_string();
            let _ = run(&db, move |conn| recipes::fail_run(conn, &run_id, &message)).await;
            return Err(AppError::from(err));
        }
    };

    let prompt = format!("[Recipe: {}] {}", recipe.title, recipe.prompt_template);
    // Immich-style routing: llm_routing.planner null → deterministic (0 tokens, planning crate)
    let is_deterministic = run(&db, |conn| {
        if let Ok(Some(v)) = finsight_core::settings::get::<serde_json::Value>(conn, "llm_routing") {
            Ok(v.get("planner").map_or(false, |x| x.is_null()))
        } else { Ok(false) }
    })
    .await
    .unwrap_or(false);
    let llm_json = if is_deterministic {
        match run(&db, {
            let prompt = prompt.clone();
            move |conn| finsight_agent::planning::answer_finance_question(conn, &prompt).map_err(|e| finsight_core::error::CoreError::InvalidState(e.to_string()))
        })
        .await
        {
            Ok(Some(answer)) => serde_json::to_value(&answer).unwrap_or(serde_json::Value::Null),
            Ok(None) => {
                let msg = "Planner: deterministic planning found no answer and LLM is disabled by routing";
                let run_id = recipe_run.id.clone();
                let _ = run(&db, move |conn| recipes::fail_run(conn, &run_id, msg)).await;
                return Err(AppError::new("recipe.planner", msg.to_string()));
            }
            Err(err) => {
                let run_id = recipe_run.id.clone();
                let message = err.to_string();
                let _ = run(&db, move |conn| recipes::fail_run(conn, &run_id, &message)).await;
                return Err(AppError::new("recipe.planner", err.to_string()));
            }
        }
    } else {
        let Some(provider) = provider_opt else {
            return Err(AppError::new(
                "no_provider",
                "Configure an AI provider in Settings → Agent before running recipes.",
            ));
        };
        match provider
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

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ListRecipeRunsRequest {
    pub recipe_id: String,
    pub limit: Option<u32>,
}

#[utoipa::path(post, path = "/api/rpc/list_recipe_runs", request_body(content = ListRecipeRunsRequest), responses((status = 200, body = Vec<AgentRecipeRun>)))]
pub async fn list_recipe_runs(
    state: &ApiState,
    recipe_id: String,
    limit: Option<u32>,
) -> AppResult<Vec<AgentRecipeRun>> {
    let db = (*state.db).clone();
    let limit = limit.unwrap_or(10);
    run(&db, move |conn| recipes::list_runs(conn, &recipe_id, limit))
        .await
        .map_err(AppError::from)
}
