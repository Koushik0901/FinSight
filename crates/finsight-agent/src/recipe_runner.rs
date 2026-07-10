use crate::reasoning::engine::ReasoningEngine;
use crate::reasoning::tools::standard_toolset;
use crate::CompletionProvider;
use anyhow::anyhow;
use finsight_core::{
    repos::{copilot_actions, recipes, run},
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

    // Run the SAME grounded tool loop the Copilot uses (standard toolset), so a
    // recipe's bundle is backed by real tool calls instead of a single-shot
    // planner prompt with the fabrication profile the eval loop measured. Draft
    // actions are staged in memory during the loop; nothing user-facing commits
    // until the bundle below is persisted (and later approved).
    let prompt = format!("[Recipe: {}] {}", recipe.title, recipe.prompt_template);
    let engine_prompt = prompt.clone();
    let engine_provider = Arc::clone(&provider);
    let result = match run(db, move |conn| {
        // ReasoningEngine::run is async and needs the pooled conn across the LLM
        // straddle — drive it on a current-thread runtime inside this blocking
        // closure, exactly as the Copilot command does.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| finsight_core::CoreError::InvalidState(format!("runtime: {e}")))?;
        rt.block_on(ReasoningEngine::run(
            conn,
            &engine_prompt,
            &standard_toolset(),
            engine_provider,
            10,
        ))
        .map_err(|e| finsight_core::CoreError::InvalidState(format!("reasoning engine: {e}")))
    })
    .await
    {
        Ok(result) => result,
        Err(err) => {
            mark_run_failed(db, recipe_run.id.clone(), err.to_string()).await;
            return Err(err.into());
        }
    };

    // Hold a reset lease across the bundle commit; skip it if a Delete-All landed
    // during the run. Delete-All drains this lease before wiping, so the bundle
    // either commits before the wipe (and is wiped) or is never written.
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
        // Persist the tool-loop output as an approvable draft bundle — the same
        // contract recipes always used, now grounded in the run's draft actions.
        let content = if result.content.trim().is_empty() {
            "Recipe analysis".to_string()
        } else {
            result.content.clone()
        };
        let reasoning = if result.reasoning.trim().is_empty() {
            "Tool-driven recipe run".to_string()
        } else {
            result.reasoning.clone()
        };
        let mut bundle = copilot_actions::insert_bundle(
            conn,
            None,
            &prompt_for_persist,
            &content,
            &reasoning,
            0.9,
            Some(&provider_id),
            Some(&model_id),
        )?;
        for (i, draft) in result.draft_actions.iter().enumerate() {
            let item = copilot_actions::insert_item(
                conn,
                &bundle.id,
                &draft.action_kind,
                &draft.payload_json,
                &draft.rationale,
                draft.confidence,
                i as i64,
            )?;
            bundle.items.push(item);
        }
        let bundle_id = bundle.id.clone();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;
    use crate::reasoning::messages::{AssistantTurn, ToolCall};
    use finsight_core::{db::run_migrations, keychain};
    use serde_json::json;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[tokio::test]
    async fn recipe_run_uses_tool_loop_and_persists_a_grounded_bundle() {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("recipe.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();

        let recipe = run(&db, |conn| {
            recipes::insert(
                conn,
                "Weekly cleanup",
                "desc",
                "cleanup",
                "Draft a planned rent payment.",
                "weekly",
                Some(1),
                None,
            )
        })
        .await
        .unwrap();

        // Script the SAME tool loop the Copilot uses: call a draft-producing tool,
        // then answer. The bundle must be backed by that tool's draft action.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!({}),
            tool_turns: Mutex::new(vec![
                AssistantTurn::ToolCalls {
                    calls: vec![ToolCall {
                        id: "c1".into(),
                        name: "draft_create_planned_transaction".into(),
                        arguments: json!({
                            "description": "Rent",
                            "amount_cents": -120000,
                            "due_date": "2026-08-01"
                        }),
                    }],
                    plan: None,
                },
                AssistantTurn::FinalAnswer {
                    content: "Drafted a planned rent payment.".to_string(),
                    reasoning: "Used the planned-transaction tool.".to_string(),
                },
            ]),
        });

        let bundle_id = run_recipe(&db, &recipe.id, provider).await.unwrap();

        let (items, run_done): (i64, i64) = run(&db, {
            let bid = bundle_id.clone();
            let rid = recipe.id.clone();
            move |conn| {
                let items: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM agent_action_items WHERE bundle_id = ?1",
                    [&bid],
                    |r| r.get(0),
                )?;
                let done: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM agent_recipe_runs \
                     WHERE recipe_id = ?1 AND bundle_id = ?2 AND status = 'completed'",
                    rusqlite::params![rid, bid],
                    |r| r.get(0),
                )?;
                Ok((items, done))
            }
        })
        .await
        .unwrap();

        assert_eq!(items, 1, "bundle is grounded in the tool call's draft action");
        assert_eq!(run_done, 1, "recipe run completed with this bundle");
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
