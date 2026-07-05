use crate::{context, planner, CompletionProvider};
use anyhow::anyhow;
use finsight_core::{
    repos::{recipes, run},
    Db,
};
use std::sync::Arc;

async fn mark_run_failed(db: &Db, run_id: String, error: String) {
    let _ = run(db, move |conn| recipes::fail_run(conn, &run_id, &error)).await;
}

pub async fn run_recipe(
    db: &Db,
    recipe_id: &str,
    provider: Arc<dyn CompletionProvider>,
) -> anyhow::Result<String> {
    let recipe_id_for_load = recipe_id.to_string();
    let recipe = run(db, move |conn| recipes::get(conn, &recipe_id_for_load)).await?;
    let recipe = recipe.ok_or_else(|| anyhow!("recipe '{recipe_id}' not found"))?;

    // Snapshot the ledger epoch before we build context + call the LLM. A recipe
    // persists an action bundle (observable Inbox state) after a seconds-long
    // LLM straddle; we hold a reset lease across that commit and skip it if a
    // Delete-All has landed, so a recipe in flight when Delete-All succeeds
    // can't leave a bundle behind against the wiped ledger.
    let start_epoch = db.reset_barrier().epoch();

    let recipe_id_for_run = recipe.id.clone();
    let recipe_run = run(db, move |conn| recipes::start_run(conn, &recipe_id_for_run)).await?;

    let ctx = match run(db, |conn| Ok(context::build_context(conn))).await {
        Ok(ctx) => ctx,
        Err(err) => {
            mark_run_failed(db, recipe_run.id.clone(), err.to_string()).await;
            return Err(err.into());
        }
    };

    let prompt = format!("[Recipe: {}] {}", recipe.title, recipe.prompt_template);
    let llm_json = match provider
        .complete_json(&planner::build_system_prompt(&ctx), &prompt)
        .await
    {
        Ok(json) => json,
        Err(err) => {
            mark_run_failed(db, recipe_run.id.clone(), err.to_string()).await;
            return Err(err);
        }
    };

    // Hold a reset lease across the plan commit; skip it if a Delete-All landed
    // during the LLM call. Delete-All drains this lease before wiping, so the
    // bundle either commits before the wipe (and is wiped) or is never written.
    let plan_lease = db.reset_barrier().writer_lease(start_epoch).await;
    if plan_lease.superseded() {
        mark_run_failed(
            db,
            recipe_run.id.clone(),
            "cancelled: data was cleared during the recipe run".to_string(),
        )
        .await;
        return Err(anyhow!("recipe cancelled: data was cleared during the run"));
    }

    let run_id = recipe_run.id.clone();
    let prompt_for_persist = prompt.clone();
    let provider_id = provider.provider_id().to_string();
    let model_id = provider.model_id().to_string();
    match run(db, move |conn| {
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
            mark_run_failed(db, recipe_run.id.clone(), err.to_string()).await;
            Err(err.into())
        }
    }
}

pub async fn run_due_recipes(
    db: &Db,
    provider: Arc<dyn CompletionProvider>,
) -> anyhow::Result<u32> {
    let due = run(db, recipes::list_due).await?;
    let attempted = due.len() as u32;
    for recipe in due {
        if let Err(err) = run_recipe(db, &recipe.id, Arc::clone(&provider)).await {
            eprintln!("trusted recipe '{}' failed: {err}", recipe.id);
        }
    }
    Ok(attempted)
}
