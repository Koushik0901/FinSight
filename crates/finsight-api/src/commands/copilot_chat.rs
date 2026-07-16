//! Copilot chat commands — persistent threaded conversations with streaming.
//!
//! Each command stores messages in the `conversations` / `conversation_messages`
//! SQLite tables (V029 migration). Streaming is simulated: the full answer is
//! produced by the reasoning engine, then emitted word-by-word as
//! `copilot-stream-frame` frames pushed through the `FrameSink` (real Tauri
//! window events on desktop, an SSE broadcast on finsight-server) so the
//! frontend sees a natural typing effect.

use crate::commands::agent::{
    build_toolset, enrich_agent_answer, is_usable_tool_answer, planner_answer_to_agent_answer,
    reasoning_result_to_agent_answer, validate_finance_answer, AgentAnswer, AgentChartBlock,
    AgentChartPoint, AgentMetricBlock, AgentRecatRow, AgentRecategorizationPreviewBlock,
    AgentResponseBlock, AgentTableBlock, AgentTxnSearchQuery,
};
use crate::error::{AppError, AppResult};
use crate::sink::FrameSink;
use crate::ApiState;
use finsight_agent::{
    planning,
    reasoning::engine::{ReasoningEngine, ReasoningEngineEvent},
};
use finsight_core::models::{ConversationMessage, ConversationSummary};
use finsight_core::repos::{conversations, run};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

// ── Public types emitted via the FrameSink as `copilot-stream-frame` ───────────

#[derive(Debug, Serialize, Clone, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CopilotStreamFrame {
    Text {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        delta: String,
    },
    Reasoning {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        reasoning_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        text: String,
    },
    ToolCallStart {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        tool_call_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        tool_name: String,
        args: Value,
    },
    ToolCallResult {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        tool_call_id: String,
        tool_result_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        result: Value,
        is_error: bool,
    },
    ResponseBlock {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        block_id: String,
        block: AgentResponseBlock,
    },
    Source {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        source_id: String,
        title: String,
    },
    Plan {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        steps: Vec<String>,
    },
    Usage {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        provider_id: String,
        model_id: String,
        elapsed_ms: u64,
        tool_count: u32,
        /// Prompt tokens the provider served from its cache this run, and the
        /// total prompt tokens, summed across every turn. 0/0 unless the active
        /// provider reports usage (OpenAI-compatible). The UI shows the cached
        /// fraction as a chip.
        cached_tokens: u32,
        prompt_tokens: u32,
    },
    Done {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        message_id: String,
        bundle_id: Option<String>,
        tool_trace: Vec<String>,
        follow_up_questions: Vec<String>,
        action_label: Option<String>,
        action_path: Option<String>,
        provider_id: String,
        model_id: String,
        elapsed_ms: u64,
        tool_count: u32,
        cached_tokens: u32,
        prompt_tokens: u32,
    },
    Error {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        code: String,
        message: String,
    },
}

// ── Input types ──────────────────────────────────────────────────────────────

/// A single prior turn from the conversation history for multi-turn awareness.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatHistoryEntry {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditConversationMessageInput {
    pub conversation_id: String,
    pub message_id: String,
    pub content: String,
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Send a message to the Copilot within a conversation.
///
/// 1. Persists the user message.
/// 2. Runs the reasoning engine (deep-mode agent pipeline).
/// 3. Streams the answer word-by-word via `copilot-stream-frame` frames pushed
///    through `sink` (a real Tauri window event on desktop, an SSE broadcast on
///    finsight-server).
/// 4. Persists the assistant message and emits the `Done` frame.
/// 5. Auto-generates a title for new conversations after the first message.
pub async fn stream_copilot_message(
    state: &ApiState,
    sink: Arc<dyn FrameSink>,
    conversation_id: String,
    run_id: String,
    text: String,
    history: Vec<ChatHistoryEntry>,
    source_message_id: Option<String>,
) -> AppResult<String> {
    let started_at = Instant::now();
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "copilot stream start conversation_id={} run_id={} chars={}",
            conversation_id,
            run_id,
            text.chars().count()
        );
    }
    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        emit_copilot_frame(
            &sink,
            CopilotStreamFrame::Error {
                conversation_id: conversation_id.clone(),
                run_id: run_id.clone(),
                thread_id: conversation_id.clone(),
                assistant_message_id: format!("assistant-{run_id}"),
                parent_message_id: source_message_id.clone(),
                sequence_number: 0,
                code: "no_provider".to_string(),
                message: "Configure an AI provider in Settings -> Agent to use this feature."
                    .to_string(),
            },
        );
        return Err(AppError::new(
            "no_provider",
            "Configure an AI provider in Settings → Agent to use this feature.",
        ));
    };

    let db = (*state.db).clone();
    let conv_id = conversation_id.clone();

    // Snapshot the ledger epoch at the start of the turn. The reasoning engine
    // reads the ledger and may end by persisting a proposed action bundle
    // (observable approval/Inbox state) after a long LLM loop — we hold a reset
    // lease across that commit and skip it if a Delete-All lands mid-turn, so a
    // turn in flight when Delete-All succeeds can't leave a bundle behind.
    let start_epoch = db.reset_barrier().epoch();

    // 1. Ensure conversation exists
    {
        let cid = conv_id.clone();
        run(&db, move |conn| {
            conversations::touch_conversation(conn, &cid)
                .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
        })
        .await
        .map_err(AppError::from)?;
    }

    // 2. Persist user message, or update the source user message for edit/reload.
    let user_parts_json = serde_json::to_string(&vec![json!({
        "type": "text",
        "text": text.clone(),
    })])
    .unwrap_or_default();
    let user_message_id = if let Some(source_id) = source_message_id.clone() {
        let cid = conv_id.clone();
        let txt = text.clone();
        let parts = user_parts_json.clone();
        run(&db, move |conn| {
            conversations::update_user_message(conn, &source_id, &txt, Some(&parts))?;
            conversations::delete_messages_after(conn, &cid, &source_id)?;
            Ok::<_, finsight_core::CoreError>(source_id)
        })
        .await
        .map_err(AppError::from)?
    } else {
        let cid = conv_id.clone();
        let txt = text.clone();
        let parts = user_parts_json.clone();
        run(&db, move |conn| {
            conversations::insert_message(conn, &cid, "user", &txt, None, None, None, Some(&parts))
                .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
        })
        .await
        .map_err(AppError::from)?
        .id
    };
    let assistant_message_id = format!("assistant-{run_id}");
    let reasoning_message_id = format!("reasoning-{run_id}");
    let parent_message_id = Some(user_message_id.clone());
    let sequence = Arc::new(AtomicU64::new(0));
    let next_sequence = {
        let sequence = Arc::clone(&sequence);
        move || sequence.fetch_add(1, Ordering::Relaxed)
    };

    // Check whether this is the very first message (for auto-titling)
    let message_count = {
        let cid = conv_id.clone();
        run(&db, move |conn| {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM conversation_messages WHERE conversation_id = ?1",
                    rusqlite::params![cid],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            Ok::<_, finsight_core::CoreError>(n)
        })
        .await
        .map_err(AppError::from)?
    };
    let is_first = source_message_id.is_none() && message_count <= 1; // only the user message just inserted

    // 3. Build enriched question with conversation history prepended
    let enriched_question = build_question_with_history(&text, &history);

    // 4. Run reasoning engine (deep mode, same as ask_agent)
    let tools = build_toolset();
    // Model tiers: with a configured fast router model, the cheap router drives
    // the tool-selection turns and the configured (strong) model synthesizes the
    // final answer; otherwise one model does both.
    let (provider_clone, synthesizer_clone) =
        match crate::provider::build_copilot_router_from_settings(&db) {
            Some(router) => (router, Some(Arc::clone(&provider))),
            None => (Arc::clone(&provider), None),
        };
    let question_for_engine = enriched_question.clone();
    let emitted_live_tool_frames = Arc::new(AtomicBool::new(false));
    emit_copilot_frame(
        &sink,
        CopilotStreamFrame::Reasoning {
            conversation_id: conv_id.clone(),
            run_id: run_id.clone(),
            thread_id: conv_id.clone(),
            assistant_message_id: assistant_message_id.clone(),
            reasoning_message_id: reasoning_message_id.clone(),
            parent_message_id: parent_message_id.clone(),
            sequence_number: next_sequence(),
            text: "Preparing local financial context and running the planning tool loop.\n"
                .to_string(),
        },
    );
    let sink_for_engine = Arc::clone(&sink);
    let conv_id_for_engine = conv_id.clone();
    let run_id_for_engine = run_id.clone();
    let assistant_message_id_for_engine = assistant_message_id.clone();
    let parent_message_id_for_engine = parent_message_id.clone();
    let sequence_for_engine = Arc::clone(&sequence);
    let live_tool_frames_for_engine = Arc::clone(&emitted_live_tool_frames);
    let command_run_id = run_id.clone();
    // Capture the arguments of the turn's `search_transactions` call so a
    // transactionTable block it produces can carry them (see the enrichment
    // after the answer is built) — that's what makes "Export as CSV" re-run the
    // exact same query instead of dumping the whole table.
    let captured_search_args = Arc::new(Mutex::new(None::<serde_json::Value>));
    let captured_search_args_for_engine = Arc::clone(&captured_search_args);
    let tool_result = run(&db, move |conn| {
        #[cfg(debug_assertions)]
        {
            eprintln!("copilot reasoning engine enter run_id={command_run_id}");
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                finsight_core::CoreError::InvalidState(format!("Failed to create runtime: {e}"))
            })?;
        let sink_for_events = Arc::clone(&sink_for_engine);
        let event_conversation_id = conv_id_for_engine.clone();
        let event_run_id = run_id_for_engine.clone();
        let event_assistant_message_id = assistant_message_id_for_engine.clone();
        let event_parent_message_id = parent_message_id_for_engine.clone();
        let event_sequence = Arc::clone(&sequence_for_engine);
        let emitted_tool_frames = Arc::clone(&live_tool_frames_for_engine);
        let captured_search_args_for_events = Arc::clone(&captured_search_args_for_engine);
        rt.block_on(async move {
            // Ceiling for the WHOLE multi-turn tool loop. 30s (tuned for small
            // fast models) silently discarded slow-but-successful runs from
            // reasoning models like Claude — a PLAN preamble plus a few tool
            // roundtrips plus a long final JSON easily exceeds it — replacing
            // the real answer with the canned planner fallback. Tool progress
            // streams live to the UI, so a longer ceiling stays visible.
            tokio::time::timeout(
                Duration::from_secs(180),
                ReasoningEngine::run_with_events(
                    conn,
                    &question_for_engine,
                    &tools,
                    provider_clone,
                    synthesizer_clone,
                    10,
                    // Internal wall-clock budget, 30s inside the outer 180s
                    // timeout: the loop synthesizes a best-effort answer at ~110s
                    // (deadline − synthesis headroom) and returns it, so the
                    // outer timeout is a rare hard safety net rather than the
                    // normal failure mode. Heavy questions degrade to a partial
                    // answer instead of a bare "timed out" error.
                    Some(std::time::Instant::now() + Duration::from_secs(150)),
                    move |event| match event {
                        ReasoningEngineEvent::PlanReady { steps } => {
                            emit_copilot_frame(
                                &sink_for_events,
                                CopilotStreamFrame::Plan {
                                    conversation_id: event_conversation_id.clone(),
                                    run_id: event_run_id.clone(),
                                    thread_id: event_conversation_id.clone(),
                                    assistant_message_id: event_assistant_message_id.clone(),
                                    parent_message_id: event_parent_message_id.clone(),
                                    sequence_number: event_sequence.fetch_add(1, Ordering::Relaxed),
                                    steps,
                                },
                            );
                        }
                        ReasoningEngineEvent::ToolCallStart { call } => {
                            if call.name == "search_transactions" {
                                if let Ok(mut slot) = captured_search_args_for_events.lock() {
                                    *slot = Some(call.arguments.clone());
                                }
                            }
                            emitted_tool_frames.store(true, Ordering::Relaxed);
                            emit_copilot_frame(
                                &sink_for_events,
                                CopilotStreamFrame::ToolCallStart {
                                    conversation_id: event_conversation_id.clone(),
                                    run_id: event_run_id.clone(),
                                    thread_id: event_conversation_id.clone(),
                                    assistant_message_id: event_assistant_message_id.clone(),
                                    tool_call_id: call.id,
                                    parent_message_id: event_parent_message_id.clone(),
                                    sequence_number: event_sequence.fetch_add(1, Ordering::Relaxed),
                                    tool_name: call.name,
                                    args: call.arguments,
                                },
                            );
                        }
                        ReasoningEngineEvent::ToolCallResult {
                            tool_call_id,
                            tool_name: _,
                            result,
                            is_error,
                        } => {
                            emit_copilot_frame(
                                &sink_for_events,
                                CopilotStreamFrame::ToolCallResult {
                                    conversation_id: event_conversation_id.clone(),
                                    run_id: event_run_id.clone(),
                                    thread_id: event_conversation_id.clone(),
                                    assistant_message_id: event_assistant_message_id.clone(),
                                    tool_result_message_id: format!("tool-result-{tool_call_id}"),
                                    tool_call_id,
                                    parent_message_id: event_parent_message_id.clone(),
                                    sequence_number: event_sequence.fetch_add(1, Ordering::Relaxed),
                                    result,
                                    is_error,
                                },
                            );
                        }
                    },
                ),
            )
            .await
            .map_err(|_| anyhow::anyhow!("Reasoning engine timed out after 180 seconds"))?
        })
        .map_err(|e| finsight_core::CoreError::InvalidState(format!("Reasoning engine error: {e}")))
    })
    .await;
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "copilot reasoning engine exit run_id={} ok={}",
            run_id,
            tool_result.is_ok()
        );
    }

    // Whether the reasoning loop had to bail to its wall-clock budget. If so we
    // stream the best-effort partial now AND spin up a background "deep answer"
    // that re-runs with a longer budget and posts a fuller follow-up.
    let mut deep_answer_needed = false;
    // Token usage summed across the run's provider turns (cache hits included),
    // captured from the ReasoningResult before it's mapped into the AgentAnswer,
    // so the Usage/Done frames and the persisted metadata can carry it.
    let mut copilot_usage = finsight_agent::TurnUsage::default();

    // 5. Build AgentAnswer from result
    let mut answer: AgentAnswer = match tool_result {
        Ok(result) if is_usable_tool_answer(&result) => {
            deep_answer_needed = result.hit_time_budget;
            copilot_usage = result.usage;
            let draft_actions = result.draft_actions.clone();
            // Kept alive past the bundle-persistence closure (which moves
            // `draft_actions`) so the recategorization preview can be synthesized
            // from the same draft data once the bundle id exists.
            let draft_actions_for_preview = draft_actions.clone();
            let question_for_db = enriched_question.clone();
            let content_for_db = result.content.clone();
            let reasoning_for_db = if result.reasoning.is_empty() {
                "Tool-driven financial analysis".to_string()
            } else {
                result.reasoning.clone()
            };
            let provider_id = provider.provider_id().to_string();
            let model_id = provider.model_id().to_string();
            // Hold a reset lease across the bundle commit; skip persisting if a
            // Delete-All landed during the turn. The wipe drains this lease
            // before running, so the bundle can't survive a completed Delete-All.
            let bundle_lease = db.reset_barrier().writer_lease(start_epoch).await;
            let bundle_id = if bundle_lease.superseded() {
                None
            } else {
                Some(
                    run(&db, move |conn| {
                        let mut bundle = finsight_core::repos::copilot_actions::insert_bundle(
                            conn,
                            None,
                            &question_for_db,
                            &content_for_db,
                            &reasoning_for_db,
                            0.9,
                            Some(&provider_id),
                            Some(&model_id),
                        )?;
                        for (i, draft) in draft_actions.iter().enumerate() {
                            let item = finsight_core::repos::copilot_actions::insert_item(
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
                        Ok::<_, finsight_core::CoreError>(bundle.id)
                    })
                    .await
                    .map_err(AppError::from)?,
                )
            };
            drop(bundle_lease);

            let mut answer = run(&db, move |conn| {
                Ok::<_, finsight_core::CoreError>(reasoning_result_to_agent_answer(
                    result, bundle_id, conn,
                ))
            })
            .await
            .map_err(AppError::from)?;
            validate_finance_answer(&enriched_question, &mut answer);
            enrich_agent_answer(&mut answer);
            // Append the recategorization preview AFTER enrich, so its presence
            // doesn't suppress the prose/reasoning fallback blocks enrich only
            // adds when response_blocks is otherwise empty. Uses answer.bundle_id
            // since `bundle_id` was moved into reasoning_result_to_agent_answer.
            if let Some(bid) = &answer.bundle_id {
                if let Some(preview_block) =
                    synthesize_recategorization_preview(&draft_actions_for_preview, bid)
                {
                    answer.response_blocks.push(preview_block);
                }
            }
            // Attach the captured search_transactions filters to any
            // transactionTable block, so its "Export as CSV" re-runs the exact
            // same query rather than exporting the whole table.
            attach_search_query_to_transaction_tables(&mut answer, &captured_search_args);
            answer
        }
        Ok(result) => {
            // The tool loop ran (and spent tokens) but didn't produce a usable
            // answer; still report its usage before falling back to the planner.
            copilot_usage = result.usage;
            // Try planner fallback
            let planned = run(&db, {
                let q = enriched_question.clone();
                move |conn| {
                    planning::answer_finance_question(conn, &q)
                        .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
                }
            })
            .await
            .map_err(AppError::from)?;

            if let Some(planned_answer) = planned {
                let mut mapped = planner_answer_to_agent_answer(planned_answer);
                mapped.trace.insert(
                    0,
                    "Tool loop incomplete; used deterministic planner fallback.".to_string(),
                );
                validate_finance_answer(&enriched_question, &mut mapped);
                enrich_agent_answer(&mut mapped);
                mapped
            } else {
                let mut answer = run(&db, move |conn| {
                    Ok::<_, finsight_core::CoreError>(reasoning_result_to_agent_answer(
                        result, None, conn,
                    ))
                })
                .await
                .map_err(AppError::from)?;
                answer.missing_data.push(
                    "The tool loop answered without the full schema; treat as provisional."
                        .to_string(),
                );
                validate_finance_answer(&enriched_question, &mut answer);
                enrich_agent_answer(&mut answer);
                answer
            }
        }
        Err(tool_err) => {
            let planned = run(&db, {
                let q = enriched_question.clone();
                move |conn| {
                    planning::answer_finance_question(conn, &q)
                        .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
                }
            })
            .await
            .map_err(AppError::from)?;

            if let Some(planned_answer) = planned {
                let mut mapped = planner_answer_to_agent_answer(planned_answer);
                mapped.trace.insert(
                    0,
                    format!("Tool loop failed; used planner fallback: {tool_err}"),
                );
                validate_finance_answer(&enriched_question, &mut mapped);
                enrich_agent_answer(&mut mapped);
                mapped
            } else if let Some(mut fallback) = run(&db, {
                let q = enriched_question.clone();
                move |conn| deterministic_copilot_fallback(conn, &q)
            })
            .await
            .map_err(AppError::from)?
            {
                fallback.trace.insert(
                    0,
                    format!("Tool loop failed; used deterministic fallback: {tool_err}"),
                );
                validate_finance_answer(&enriched_question, &mut fallback);
                enrich_agent_answer(&mut fallback);
                fallback
            } else {
                let safe = "The Copilot could not complete this request. Check the AI provider and model in Settings → Agent, then try again.";
                emit_stream_error(
                    &sink,
                    &conv_id,
                    &run_id,
                    &assistant_message_id,
                    &parent_message_id,
                    next_sequence(),
                    "agent.reasoning",
                    safe,
                    &tool_err.to_string(),
                );
                persist_failed_run(&db, &conv_id, &run_id, "agent.reasoning", safe).await;
                return Err(AppError::new("agent.reasoning", safe));
            }
        }
    };

    if answer.prose.trim().is_empty() {
        let planned = run(&db, {
            let q = enriched_question.clone();
            move |conn| {
                planning::answer_finance_question(conn, &q)
                    .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
            }
        })
        .await
        .map_err(AppError::from)?;

        if let Some(planned_answer) = planned {
            let mut mapped = planner_answer_to_agent_answer(planned_answer);
            mapped.trace.insert(
                0,
                "Reasoning returned empty prose; used planner fallback.".to_string(),
            );
            validate_finance_answer(&enriched_question, &mut mapped);
            enrich_agent_answer(&mut mapped);
            answer = mapped;
        }

        if answer.prose.trim().is_empty() {
            let safe = "Copilot finished without a text response. Check the configured AI provider/model in Settings → Agent, then try again.";
            emit_stream_error(
                &sink,
                &conv_id,
                &run_id,
                &assistant_message_id,
                &parent_message_id,
                next_sequence(),
                "agent.empty_response",
                safe,
                "reasoning + planner both returned empty prose",
            );
            persist_failed_run(&db, &conv_id, &run_id, "agent.empty_response", safe).await;
            return Err(AppError::new("agent.empty_response", safe));
        }
    }

    // Heavy question that bailed to the time budget: note it on the quick answer
    // and kick off a detached background "deep answer" (OpenAI background-mode
    // style) that re-runs on the strong model with a longer budget and posts a
    // fuller follow-up assistant message the UI surfaces via copilot-async-answer.
    if deep_answer_needed {
        answer.prose.push_str(
            "\n\n_⏳ That was a quick read under a time limit — I'm working on a fuller analysis and will post it here shortly._",
        );
        spawn_deep_answer(
            Arc::clone(&sink),
            db.clone(),
            Arc::clone(&provider),
            conv_id.clone(),
            enriched_question.clone(),
        );
    }

    let provider_id = provider.provider_id().to_string();
    let model_id = provider.model_id().to_string();
    let tool_names = tool_names_from_trace(&answer.trace);
    let already_emitted_tool_frames = emitted_live_tool_frames.load(Ordering::Relaxed);

    // 6. Emit rich assistant-ui parts before the final text stream.
    if !answer.reasoning.trim().is_empty() {
        emit_copilot_frame(
            &sink,
            CopilotStreamFrame::Reasoning {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                thread_id: conv_id.clone(),
                assistant_message_id: assistant_message_id.clone(),
                reasoning_message_id: reasoning_message_id.clone(),
                parent_message_id: parent_message_id.clone(),
                sequence_number: next_sequence(),
                text: answer.reasoning.clone(),
            },
        );
    }

    if !already_emitted_tool_frames {
        for (i, tool_name) in tool_names.iter().enumerate() {
            let tool_call_id = format!("tool-{i}");
            emit_copilot_frame(
                &sink,
                CopilotStreamFrame::ToolCallStart {
                    conversation_id: conv_id.clone(),
                    run_id: run_id.clone(),
                    thread_id: conv_id.clone(),
                    assistant_message_id: assistant_message_id.clone(),
                    tool_call_id: tool_call_id.clone(),
                    parent_message_id: parent_message_id.clone(),
                    sequence_number: next_sequence(),
                    tool_name: tool_name.clone(),
                    args: json!({}),
                },
            );
            emit_copilot_frame(
                &sink,
                CopilotStreamFrame::ToolCallResult {
                    conversation_id: conv_id.clone(),
                    run_id: run_id.clone(),
                    thread_id: conv_id.clone(),
                    assistant_message_id: assistant_message_id.clone(),
                    tool_result_message_id: format!("tool-result-{tool_call_id}"),
                    tool_call_id,
                    parent_message_id: parent_message_id.clone(),
                    sequence_number: next_sequence(),
                    result: json!({
                        "ok": true,
                        "summary": answer.trace.get(i).cloned().unwrap_or_else(|| tool_name.clone()),
                    }),
                    is_error: false,
                },
            );
        }
    }

    for (i, block) in answer
        .response_blocks
        .iter()
        .filter(|block| should_emit_response_block(block))
        .filter(|block| response_block_within_artifact_bounds(block))
        .cloned()
        .enumerate()
    {
        emit_copilot_frame(
            &sink,
            CopilotStreamFrame::ResponseBlock {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                thread_id: conv_id.clone(),
                assistant_message_id: assistant_message_id.clone(),
                parent_message_id: parent_message_id.clone(),
                sequence_number: next_sequence(),
                block_id: format!("block-{i}"),
                block,
            },
        );
    }

    for (i, title) in answer.data_sources.iter().cloned().enumerate() {
        emit_copilot_frame(
            &sink,
            CopilotStreamFrame::Source {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                thread_id: conv_id.clone(),
                assistant_message_id: assistant_message_id.clone(),
                parent_message_id: parent_message_id.clone(),
                sequence_number: next_sequence(),
                source_id: format!("source-{i}"),
                title,
            },
        );
    }

    // 7. Simulated text streaming: emit prose in word-sized chunks at ~25 ms
    // each for a natural typing effect. Split on whitespace WITHOUT discarding
    // it (`split_inclusive`, not `split_whitespace`) so the chunks concatenate
    // back to the exact prose — newlines included. Using `split_whitespace`
    // here collapsed every `\n` into a single space, so the answer's markdown
    // (headings, horizontal rules, GFM tables, lists) arrived as one giant
    // line and rendered as raw `##` / `---` / `|` text instead of formatting.
    for chunk in answer.prose.split_inclusive(char::is_whitespace) {
        emit_copilot_frame(
            &sink,
            CopilotStreamFrame::Text {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                thread_id: conv_id.clone(),
                assistant_message_id: assistant_message_id.clone(),
                parent_message_id: parent_message_id.clone(),
                sequence_number: next_sequence(),
                delta: chunk.to_string(),
            },
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
    }

    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    emit_copilot_frame(
        &sink,
        CopilotStreamFrame::Usage {
            conversation_id: conv_id.clone(),
            run_id: run_id.clone(),
            thread_id: conv_id.clone(),
            assistant_message_id: assistant_message_id.clone(),
            parent_message_id: parent_message_id.clone(),
            sequence_number: next_sequence(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
            elapsed_ms,
            tool_count: tool_names.len() as u32,
            cached_tokens: copilot_usage.cached_tokens,
            prompt_tokens: copilot_usage.prompt_tokens,
        },
    );

    // 8. Persist assistant message
    let assistant_prose = answer.prose.clone();
    let bundle_id_for_db = answer.bundle_id.clone();
    let trace_json = serde_json::to_string(&answer.trace).unwrap_or_default();
    let parts_json = assistant_parts_json(&answer);
    let ag_ui_metadata_json = serde_json::to_string(&json!({
        "schemaVersion": 1,
        "runtime": "ag-ui",
        "runId": run_id.clone(),
        "threadId": conv_id.clone(),
        "assistantMessageId": assistant_message_id.clone(),
        "parentMessageId": parent_message_id.clone(),
        "runStatus": "completed",
        "providerId": provider_id.clone(),
        "modelId": model_id.clone(),
        "elapsedMs": elapsed_ms,
        "toolCount": tool_names.len(),
        "cachedTokens": copilot_usage.cached_tokens,
        "promptTokens": copilot_usage.prompt_tokens,
        "bundleId": answer.bundle_id.clone(),
        "toolTrace": answer.trace.clone(),
        "plan": answer.plan.clone(),
        "followUpQuestions": answer.follow_up_questions.clone(),
        "actionLabel": answer.action_label.clone(),
        "actionPath": answer.action_path.clone(),
    }))
    .unwrap_or_default();
    let asst_msg = {
        let cid = conv_id.clone();
        let parts = parts_json.clone();
        let metadata = ag_ui_metadata_json.clone();
        run(&db, move |conn| {
            let message = conversations::insert_message(
                conn,
                &cid,
                "assistant",
                &assistant_prose,
                Some(trace_json.as_str()),
                bundle_id_for_db.as_deref(),
                None,
                Some(&parts),
            )?;
            conversations::update_message_run_status(
                conn,
                &message.id,
                "completed",
                Some(&metadata),
            )?;
            Ok(message)
        })
        .await
        .map_err(AppError::from)?
    };
    let asst_msg_id = asst_msg.id.clone();

    // 9. Emit completion frame.
    emit_copilot_frame(
        &sink,
        CopilotStreamFrame::Done {
            conversation_id: conv_id.clone(),
            run_id: run_id.clone(),
            thread_id: conv_id.clone(),
            assistant_message_id: assistant_message_id.clone(),
            parent_message_id: parent_message_id.clone(),
            sequence_number: next_sequence(),
            message_id: asst_msg_id.clone(),
            bundle_id: answer.bundle_id.clone(),
            tool_trace: answer.trace.clone(),
            follow_up_questions: answer.follow_up_questions.clone(),
            action_label: answer.action_label.clone(),
            action_path: answer.action_path.clone(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
            elapsed_ms,
            tool_count: tool_names.len() as u32,
            cached_tokens: copilot_usage.cached_tokens,
            prompt_tokens: copilot_usage.prompt_tokens,
        },
    );

    // 10. Auto-generate title for new conversations
    if is_first {
        let provider_clone = Arc::clone(&provider);
        let text_clone = text.clone();
        let prose_clone = answer.prose.clone();
        let cid = conv_id.clone();
        let db_clone = db.clone();
        tokio::spawn(async move {
            let system = "Generate a short 4-6 word title for this conversation. \
                          Respond with JSON only: {\"title\": \"...\"}. \
                          No punctuation at the end. No quotes around the title. \
                          Be specific to the financial topic.";
            let prompt = format!(
                "User asked: {}\nAssistant replied: {}",
                &text_clone,
                &prose_clone.chars().take(200).collect::<String>()
            );
            if let Ok(v) = provider_clone.complete_json(system, &prompt).await {
                if let Some(title) = v.get("title").and_then(|t| t.as_str()) {
                    let title = title.to_string();
                    let _ = run(&db_clone, move |conn| {
                        conversations::update_conversation_title(conn, &cid, &title)
                            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
                    })
                    .await;
                }
            }
        });
    }

    Ok(conv_id)
}

pub async fn list_conversations(state: &ApiState) -> AppResult<Vec<ConversationSummary>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        conversations::list_conversations(conn)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
}

pub async fn get_conversation_messages(
    state: &ApiState,
    conversation_id: String,
) -> AppResult<Vec<ConversationMessage>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        conversations::list_messages(conn, &conversation_id)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
}

pub async fn delete_conversation(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        conversations::delete_conversation(conn, &id)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
}

pub async fn create_conversation(state: &ApiState) -> AppResult<String> {
    let db = (*state.db).clone();
    let id = uuid::Uuid::new_v4().to_string();
    run(&db, move |conn| {
        conversations::create_conversation(conn, &id)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
    .map(|s| s.id)
}

pub async fn edit_conversation_user_message(
    state: &ApiState,
    input: EditConversationMessageInput,
) -> AppResult<()> {
    let db = (*state.db).clone();
    let parts_json = serde_json::to_string(&vec![json!({
        "type": "text",
        "text": input.content.clone(),
    })])
    .unwrap_or_default();
    run(&db, move |conn| {
        conversations::update_user_message(
            conn,
            &input.message_id,
            &input.content,
            Some(&parts_json),
        )?;
        conversations::delete_messages_after(conn, &input.conversation_id, &input.message_id)?;
        Ok::<_, finsight_core::CoreError>(())
    })
    .await
    .map_err(AppError::from)
}

pub async fn delete_conversation_messages_after(
    state: &ApiState,
    conversation_id: String,
    message_id: String,
) -> AppResult<u32> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        conversations::delete_messages_after(conn, &conversation_id, &message_id).map(|n| n as u32)
    })
    .await
    .map_err(AppError::from)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn emit_copilot_frame(sink: &Arc<dyn FrameSink>, frame: CopilotStreamFrame) {
    sink.emit(
        "copilot-stream-frame",
        serde_json::to_value(frame).expect("frame serializes"),
    );
}

/// Emit a Copilot `Error` stream frame with a UI-safe message. The raw error
/// (which may contain provider URLs, response bodies, or other internals) is
/// only logged locally in debug builds — never sent to the frontend. This keeps
/// Task-4 guarantees: no raw stack traces, provider errors, URLs, or secrets in
/// the UI.
#[allow(clippy::too_many_arguments)]
fn emit_stream_error(
    sink: &Arc<dyn FrameSink>,
    conv_id: &str,
    run_id: &str,
    assistant_message_id: &str,
    parent_message_id: &Option<String>,
    sequence_number: u64,
    code: &str,
    safe_message: &str,
    raw_detail: &str,
) {
    #[cfg(debug_assertions)]
    {
        eprintln!("copilot stream error code={code} run_id={run_id} detail={raw_detail}");
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = raw_detail;
    }
    emit_copilot_frame(
        sink,
        CopilotStreamFrame::Error {
            conversation_id: conv_id.to_string(),
            run_id: run_id.to_string(),
            thread_id: conv_id.to_string(),
            assistant_message_id: assistant_message_id.to_string(),
            parent_message_id: parent_message_id.clone(),
            sequence_number,
            code: code.to_string(),
            message: safe_message.to_string(),
        },
    );
}

/// Persist a durable "failed" assistant turn so a reloaded conversation can tell
/// a failed run apart from a completed one (Task 7 reload safety). Best-effort:
/// a persistence failure here must not mask the original error.
async fn persist_failed_run(
    db: &finsight_core::Db,
    conv_id: &str,
    run_id: &str,
    code: &str,
    safe_message: &str,
) {
    let metadata = serde_json::to_string(&json!({
        "schemaVersion": 1,
        "runtime": "ag-ui",
        "runId": run_id,
        "threadId": conv_id,
        "runStatus": "failed",
        "errorCode": code,
    }))
    .unwrap_or_default();
    let cid = conv_id.to_string();
    let msg = safe_message.to_string();
    let meta = metadata.clone();
    let _ = run(db, move |conn| {
        let message =
            conversations::insert_message(conn, &cid, "assistant", &msg, None, None, None, None)?;
        conversations::update_message_run_status(conn, &message.id, "failed", Some(&meta))?;
        Ok::<_, finsight_core::CoreError>(())
    })
    .await;
}

fn tool_names_from_trace(trace: &[String]) -> Vec<String> {
    trace
        .iter()
        .filter_map(|entry| {
            entry
                .strip_prefix("Called tool:")
                .or_else(|| entry.strip_prefix("Called tool"))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| name.trim_matches(':').trim().to_string())
        })
        .collect()
}

/// Detached background "deep answer" (OpenAI background-mode style): re-run the
/// reasoning loop on the strong model with a longer, off-the-request-path budget
/// and persist a fuller follow-up assistant message, then notify the UI via a
/// `copilot-async-answer` frame pushed through `sink`. Triggered when the
/// synchronous turn had to bail to its wall-clock budget. Best-effort: any
/// failure is silently dropped (the user already has the quick partial answer).
///
/// The original Tauri-only implementation also fired a native OS notification
/// ("Copilot finished a deeper analysis") once this landed. That call used
/// `tauri_plugin_notification` directly and can't move into tauri-free
/// finsight-api (and `FrameSink`/`TauriFrameSink` are deliberately not extended
/// for it — see Task 6 of the server-phase1-skeleton plan). It is dropped here,
/// same as the plan's stated rule that native notifications don't exist in
/// Phase 1's transport-agnostic core; the `copilot-async-answer` event (which the
/// UI actually renders from) is fully preserved via `sink.emit`.
fn spawn_deep_answer(
    sink: Arc<dyn FrameSink>,
    db: finsight_core::Db,
    provider: Arc<dyn finsight_agent::CompletionProvider>,
    conversation_id: String,
    question: String,
) {
    tokio::spawn(async move {
        // Captured before `provider` is moved into the run closure, so the
        // persisted metadata can name the model and report the cache usage.
        let started = Instant::now();
        let provider_id = provider.provider_id().to_string();
        let model_id = provider.model_id().to_string();
        let tools = build_toolset();
        let q = question.clone();
        let deep = run(&db, move |conn| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| {
                    finsight_core::CoreError::InvalidState(format!("deep-answer rt: {e}"))
                })?;
            // 8-minute budget, no synchronous wall: the loop still degrades
            // gracefully at the budget rather than running unbounded.
            rt.block_on(ReasoningEngine::run_with_events(
                conn,
                &q,
                &tools,
                provider,
                None,
                15,
                Some(Instant::now() + Duration::from_secs(480)),
                |_| {},
            ))
            .map_err(|e| finsight_core::CoreError::InvalidState(format!("deep answer: {e}")))
        })
        .await;

        let Ok(result) = deep else { return };
        if !is_usable_tool_answer(&result) {
            return;
        }
        // Same answer-construction pipeline as the synchronous turn, so the deep
        // follow-up renders identically — including the model's own structured
        // blocks (tables/charts/verdicts), which reasoning_result_to_agent_answer
        // now maps. The deep answer persists no applyable action bundle, so drop
        // any drafted change entries (they'd have no bundle to apply against).
        // Captured before `result` is moved into the answer mapping.
        let deep_usage = result.usage;
        let Ok(mut answer) = run(&db, move |conn| {
            Ok::<_, finsight_core::CoreError>(reasoning_result_to_agent_answer(result, None, conn))
        })
        .await
        else {
            return;
        };
        answer.changes = Vec::new();
        validate_finance_answer(&question, &mut answer);
        enrich_agent_answer(&mut answer);

        // Persist the same meta the synchronous turn writes, so the deep answer
        // shows its model/timing/tool-count — and the prompt-cache chip — on
        // reload (buildMetaFromMessages reads these back from agUiMetadataJson).
        let tool_count = tool_names_from_trace(&answer.trace).len();
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let ag_ui_metadata_json = serde_json::to_string(&json!({
            "schemaVersion": 1,
            "runtime": "ag-ui",
            "threadId": conversation_id.clone(),
            "runStatus": "completed",
            "providerId": provider_id,
            "modelId": model_id,
            "elapsedMs": elapsed_ms,
            "toolCount": tool_count,
            "cachedTokens": deep_usage.cached_tokens,
            "promptTokens": deep_usage.prompt_tokens,
            "toolTrace": answer.trace.clone(),
            "plan": answer.plan.clone(),
            "followUpQuestions": answer.follow_up_questions.clone(),
        }))
        .unwrap_or_default();

        let parts = assistant_parts_json(&answer);
        let prose = answer.prose.clone();
        let cid = conversation_id.clone();
        let persisted = run(&db, move |conn| {
            let msg = conversations::insert_message(
                conn, &cid, "assistant", &prose, None, None, None, Some(&parts),
            )?;
            conversations::update_message_run_status(
                conn,
                &msg.id,
                "completed",
                Some(&ag_ui_metadata_json),
            )?;
            Ok::<_, finsight_core::CoreError>(msg.id)
        })
        .await;
        let Ok(message_id) = persisted else { return };
        sink.emit(
            "copilot-async-answer",
            json!({
                "conversationId": conversation_id,
                "messageId": message_id,
                "prose": answer.prose,
            }),
        );
        // Native OS notification dropped here — see the doc comment on this
        // function for why.
    });
}

fn assistant_parts_json(answer: &AgentAnswer) -> String {
    let mut parts = Vec::new();

    if !answer.reasoning.trim().is_empty() {
        parts.push(json!({
            "type": "reasoning",
            "text": answer.reasoning,
        }));
    }

    for (i, tool_name) in tool_names_from_trace(&answer.trace).into_iter().enumerate() {
        parts.push(json!({
            "type": "tool-call",
            "toolCallId": format!("tool-{i}"),
            "toolName": tool_name,
            "args": {},
            "argsText": "{}",
            "result": {
                "ok": true,
                "summary": answer.trace.get(i).cloned().unwrap_or_default(),
            },
        }));
    }

    for (i, block) in answer
        .response_blocks
        .iter()
        .filter(|block| should_emit_response_block(block))
        .enumerate()
    {
        parts.push(response_block_part(format!("block-{i}"), block));
    }

    for (i, source) in answer.data_sources.iter().enumerate() {
        parts.push(json!({
            "type": "source",
            "sourceType": "document",
            "id": format!("source-{i}"),
            "title": source,
            "mediaType": "application/x-finsight-source",
        }));
    }

    parts.push(json!({
        "type": "text",
        "text": answer.prose,
    }));

    serde_json::to_string(&parts).unwrap_or_else(|_| {
        serde_json::to_string(&vec![json!({"type": "text", "text": answer.prose})])
            .unwrap_or_default()
    })
}

/// Synthesizes a `RecategorizationPreview` block from a turn's draft actions.
/// This is the ONE artifact kind the model never chooses via response_blocks —
/// its bundle_id only exists after the action bundle is persisted (see
/// insert_bundle/insert_item above), which happens after the reasoning loop
/// returns. Reads the same preview data `draft_recategorization` already
/// computed (act.rs) straight out of the draft action's payload_json.
fn synthesize_recategorization_preview(
    draft_actions: &[finsight_agent::reasoning::messages::AgentDraftAction],
    bundle_id: &str,
) -> Option<AgentResponseBlock> {
    let draft = draft_actions
        .iter()
        .find(|d| d.action_kind == "recategorize_bulk")?;
    let payload: serde_json::Value = serde_json::from_str(&draft.payload_json).ok()?;
    let assignments = payload.get("assignments")?.as_array()?;
    if assignments.is_empty() {
        return None;
    }

    const PREVIEW_ROWS: usize = 5;
    let rows: Vec<AgentRecatRow> = assignments
        .iter()
        .take(PREVIEW_ROWS)
        .filter_map(|a| {
            Some(AgentRecatRow {
                merchant: a.get("merchant")?.as_str()?.to_string(),
                category_key: a.get("categoryLabel")?.as_str()?.to_string(),
                confidence: a
                    .get("confidence")
                    .and_then(|c| c.as_f64())
                    .unwrap_or(0.7)
                    .clamp(0.0, 1.0),
            })
        })
        .collect();
    if rows.is_empty() {
        return None;
    }

    Some(AgentResponseBlock::RecategorizationPreview(
        AgentRecategorizationPreviewBlock {
            count: assignments.len() as i64,
            more: (assignments.len().saturating_sub(rows.len())) as i64,
            rows,
            bundle_id: bundle_id.to_string(),
        },
    ))
}

/// Maps a raw `search_transactions` tool-call arguments object into the typed
/// query the transactionTable block carries. Mirrors the tool's own argument
/// reading (see `read.rs`), including dropping a `"any"` direction.
fn search_query_from_args(args: &Value) -> AgentTxnSearchQuery {
    AgentTxnSearchQuery {
        merchant: args["merchant"].as_str().map(String::from),
        account: args["account"].as_str().map(String::from),
        start_date: args["start_date"].as_str().map(String::from),
        end_date: args["end_date"].as_str().map(String::from),
        min_amount_cents: args["min_amount_cents"].as_i64(),
        direction: args["direction"]
            .as_str()
            .filter(|d| *d != "any")
            .map(String::from),
    }
}

/// Attaches the turn's captured `search_transactions` filters to every
/// transactionTable block that doesn't already carry a query, so the card's CSV
/// export re-runs the exact query that produced the table.
///
/// Assumes the common ~1:1 table-to-search relationship the system prompt
/// encourages: only the last `search_transactions` call's args are captured, so
/// a turn that produced multiple tables from multiple distinct searches would
/// attach the last search's filters to all of them. Acceptable given how rare
/// that shape is; revisit if multi-search turns become common.
fn attach_search_query_to_transaction_tables(
    answer: &mut AgentAnswer,
    captured_search_args: &Mutex<Option<Value>>,
) {
    let Ok(slot) = captured_search_args.lock() else {
        return;
    };
    let Some(args) = slot.as_ref() else {
        return;
    };
    let query = search_query_from_args(args);
    for block in answer.response_blocks.iter_mut() {
        if let AgentResponseBlock::TransactionTable(t) = block {
            if t.query.is_none() {
                t.query = Some(query.clone());
            }
        }
    }
}

fn response_block_part(id: String, block: &AgentResponseBlock) -> Value {
    json!({
        "type": "generative-ui",
        "id": id,
        "spec": {
            "root": {
                "component": "FinSightResponseBlock",
                "props": {
                    "block": block,
                }
            }
        }
    })
}

fn should_emit_response_block(block: &AgentResponseBlock) -> bool {
    match block {
        AgentResponseBlock::Markdown { .. } => false,
        AgentResponseBlock::Callout { title, .. } => title.as_deref() != Some("Reasoning"),
        AgentResponseBlock::Table(_)
        | AgentResponseBlock::BarChart(_)
        | AgentResponseBlock::LineChart(_)
        | AgentResponseBlock::MetricGrid { .. } => true,
        AgentResponseBlock::TransactionTable(_) => true,
        AgentResponseBlock::AffordabilityVerdict(_) => true,
        AgentResponseBlock::CategoryBreakdown(_) => true,
        AgentResponseBlock::AllocationSplit(_) => true,
        AgentResponseBlock::RankedOptions(_) => true,
        AgentResponseBlock::ComparisonBars(_) => true,
        AgentResponseBlock::RecategorizationPreview(_) => true,
        AgentResponseBlock::SpendingReview(_) => true,
        AgentResponseBlock::AccountsOverview(_) => true,
        AgentResponseBlock::SpendTimeline(_) => true,
        AgentResponseBlock::SpendingDrivers(_) => true,
        AgentResponseBlock::WatchList(_) => true,
        AgentResponseBlock::ActionPlan(_) => true,
    }
}

// Artifact bounds — kept in lockstep with `ui/src/components/copilot/agUi/artifacts.ts`
// so the backend never emits a `ResponseBlock` the frontend would reject as
// oversized/malformed. A block that violates a bound is simply not emitted as an
// artifact (the prose still carries the information).
const ARTIFACT_MAX_TABLE_ROWS: usize = 200;
const ARTIFACT_MAX_TABLE_COLS: usize = 24;
const ARTIFACT_MAX_METRICS: usize = 50;
const ARTIFACT_MAX_CHART_POINTS: usize = 200;
const ARTIFACT_MAX_TEXT: usize = 20_000;
const ARTIFACT_MAX_LABEL: usize = 400;

fn label_ok(value: &str) -> bool {
    value.chars().count() <= ARTIFACT_MAX_LABEL
}

fn opt_label_ok(value: &Option<String>) -> bool {
    value.as_deref().map(label_ok).unwrap_or(true)
}

/// True when a response block is safe to emit as a finance artifact: within all
/// size bounds and free of non-finite chart values. Mirrors the TypeScript
/// `CopilotResponseBlockSchema` validation on the receiving end.
fn response_block_within_artifact_bounds(block: &AgentResponseBlock) -> bool {
    match block {
        AgentResponseBlock::Markdown { markdown } => markdown.chars().count() <= ARTIFACT_MAX_TEXT,
        AgentResponseBlock::Callout { tone, title, body } => {
            label_ok(tone) && opt_label_ok(title) && body.chars().count() <= ARTIFACT_MAX_TEXT
        }
        AgentResponseBlock::Table(t) => {
            opt_label_ok(&t.title)
                && t.columns.len() <= ARTIFACT_MAX_TABLE_COLS
                && t.columns.iter().all(|c| label_ok(c))
                && t.rows.len() <= ARTIFACT_MAX_TABLE_ROWS
                && t.rows
                    .iter()
                    .all(|r| r.len() <= ARTIFACT_MAX_TABLE_COLS && r.iter().all(|c| label_ok(c)))
        }
        AgentResponseBlock::BarChart(c) | AgentResponseBlock::LineChart(c) => {
            opt_label_ok(&c.title)
                && opt_label_ok(&c.series_label)
                && c.data.len() <= ARTIFACT_MAX_CHART_POINTS
                && c.data
                    .iter()
                    .all(|p| label_ok(&p.label) && p.value.is_finite())
        }
        AgentResponseBlock::MetricGrid { metrics } => {
            metrics.len() <= ARTIFACT_MAX_METRICS
                && metrics.iter().all(|m| {
                    label_ok(&m.label)
                        && label_ok(&m.value)
                        && opt_label_ok(&m.detail)
                        && opt_label_ok(&m.tone)
                })
        }
        AgentResponseBlock::TransactionTable(t) => {
            t.rows.len() <= ARTIFACT_MAX_TABLE_ROWS
                && t.rows.iter().all(|r| {
                    label_ok(&r.merchant)
                        && label_ok(&r.category_key)
                        && opt_label_ok(&r.flag)
                        && r.date.len() <= ARTIFACT_MAX_LABEL
                })
        }
        AgentResponseBlock::AffordabilityVerdict(v) => {
            label_ok(&v.headline)
                && label_ok(&v.sub)
                && opt_label_ok(&v.caveat)
                && v.funding_source
                    .as_ref()
                    .map_or(true, |f| label_ok(&f.label) && label_ok(&f.detail))
        }
        AgentResponseBlock::CategoryBreakdown(b) => {
            label_ok(&b.period_label)
                && b.rows.len() <= 30
                && b.rows.iter().all(|r| label_ok(&r.category_key))
        }
        AgentResponseBlock::AllocationSplit(b) => {
            b.segments.len() <= 12
                && b.segments.iter().all(|s| {
                    label_ok(&s.label) && label_ok(&s.rationale) && label_ok(&s.category_key)
                })
        }
        AgentResponseBlock::RankedOptions(b) => {
            label_ok(&b.title)
                && b.options.len() <= 10
                && b.options
                    .iter()
                    .all(|o| label_ok(&o.label) && label_ok(&o.detail) && label_ok(&o.rationale))
        }
        AgentResponseBlock::ComparisonBars(b) => {
            label_ok(&b.title) && label_ok(&b.current.label) && label_ok(&b.prior.label)
        }
        AgentResponseBlock::RecategorizationPreview(b) => {
            b.rows.len() <= 20
                && b.rows.iter().all(|r| label_ok(&r.merchant) && label_ok(&r.category_key))
                && label_ok(&b.bundle_id)
        }
        AgentResponseBlock::SpendingReview(b) => {
            b.months.len() <= 6
                && b.months.iter().all(|m| {
                    label_ok(&m.label)
                        && opt_label_ok(&m.subtitle)
                        && m.summary
                            .as_deref()
                            .map(|s| s.chars().count() <= ARTIFACT_MAX_TEXT)
                            .unwrap_or(true)
                        && m.categories.len() <= 10
                        && m.categories
                            .iter()
                            .all(|c| label_ok(&c.label) && opt_label_ok(&c.tag))
                        && m.actions.len() <= 6
                        && m.actions.iter().all(|a| label_ok(a))
                })
        }
        AgentResponseBlock::AccountsOverview(b) => {
            opt_label_ok(&b.title)
                && opt_label_ok(&b.subtitle)
                && b.rows.len() <= 30
                && b.rows.iter().all(|r| {
                    label_ok(&r.name)
                        && opt_label_ok(&r.subtitle)
                        && label_ok(&r.type_label)
                        && opt_label_ok(&r.badge)
                })
        }
        AgentResponseBlock::SpendTimeline(b) => {
            opt_label_ok(&b.title)
                && opt_label_ok(&b.subtitle)
                && b.points.len() <= 24
                && b.points
                    .iter()
                    .all(|p| label_ok(&p.label) && opt_label_ok(&p.annotation))
        }
        AgentResponseBlock::SpendingDrivers(b) => {
            label_ok(&b.title)
                && opt_label_ok(&b.subtitle)
                && b.drivers.len() <= 8
                && b.drivers.iter().all(|d| {
                    label_ok(&d.label)
                        && label_ok(&d.tag)
                        && label_ok(&d.amount_display)
                        && opt_label_ok(&d.note)
                })
        }
        AgentResponseBlock::WatchList(b) => {
            label_ok(&b.title)
                && b.items.len() <= 8
                && b.items.iter().all(|it| {
                    label_ok(&it.label)
                        && it.detail.chars().count() <= ARTIFACT_MAX_TEXT
                        && opt_label_ok(&it.amount_display)
                })
        }
        AgentResponseBlock::ActionPlan(b) => {
            opt_label_ok(&b.title) && b.items.len() <= 8 && b.items.iter().all(|i| label_ok(i))
        }
    }
}

fn deterministic_copilot_fallback(
    conn: &mut rusqlite::Connection,
    question: &str,
) -> Result<Option<AgentAnswer>, finsight_core::CoreError> {
    let q = question.to_lowercase();
    let asks_spending = (q.contains("spend") || q.contains("spent") || q.contains("expense"))
        && (q.contains("most")
            || q.contains("top")
            || q.contains("category")
            || q.contains("month"));
    if !asks_spending {
        return Ok(None);
    }

    let rows = top_spending_categories_this_month(conn)
        .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))?;
    if rows.is_empty() {
        return Ok(Some(AgentAnswer {
            prose: "I could not find cleared spending transactions for the current month. If this looks wrong, check the transaction dates, account sync status, and whether expenses are imported as negative amounts.".to_string(),
            reasoning: "The deterministic fallback queried current-month negative transactions grouped by category and found no rows.".to_string(),
            plan: Vec::new(),
            trace: vec!["Called tool: get_top_spending_categories".to_string()],
            changes: Vec::new(),
            action_label: None,
            action_path: None,
            bundle_id: None,
            assumptions: vec![
                "Current month is calculated from the local database clock.".to_string(),
                "Expenses are treated as negative transaction amounts.".to_string(),
            ],
            data_sources: vec!["Local transactions table".to_string()],
            missing_data: vec!["No current-month expense rows were found.".to_string()],
            alternatives: Vec::new(),
            follow_up_questions: vec![
                "Show the largest individual transactions this month.".to_string(),
                "Compare this month against last month.".to_string(),
            ],
            response_blocks: Vec::new(),
        }));
    }

    let total_cents: i64 = rows.iter().map(|row| row.amount_cents).sum();
    let top = &rows[0];
    let prose = format!(
        "Your largest spending category this month is **{}** at {}, across {} transaction{}. That is about {:.0}% of the categorized spending I found for the month.",
        top.category,
        format_cents(top.amount_cents),
        top.transaction_count,
        if top.transaction_count == 1 { "" } else { "s" },
        if total_cents > 0 {
            (top.amount_cents as f64 / total_cents as f64) * 100.0
        } else {
            0.0
        }
    );

    let table = AgentResponseBlock::Table(AgentTableBlock {
        title: Some("Top spending categories this month".to_string()),
        columns: vec![
            "Category".to_string(),
            "Spent".to_string(),
            "Transactions".to_string(),
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.category.clone(),
                    format_cents(row.amount_cents),
                    row.transaction_count.to_string(),
                ]
            })
            .collect(),
    });
    let chart = AgentResponseBlock::BarChart(AgentChartBlock {
        title: Some("Spending by category".to_string()),
        series_label: Some("Spent".to_string()),
        data: rows
            .iter()
            .map(|row| AgentChartPoint {
                label: row.category.clone(),
                value: row.amount_cents as f64 / 100.0,
            })
            .collect(),
    });
    let metrics = AgentResponseBlock::MetricGrid {
        metrics: vec![
            AgentMetricBlock {
                label: "Top category".to_string(),
                value: top.category.clone(),
                detail: Some(format!(
                    "{} transaction{}",
                    top.transaction_count,
                    if top.transaction_count == 1 { "" } else { "s" }
                )),
                tone: Some("neutral".to_string()),
            },
            AgentMetricBlock {
                label: "Top category spend".to_string(),
                value: format_cents(top.amount_cents),
                detail: Some("Current month".to_string()),
                tone: Some("warning".to_string()),
            },
            AgentMetricBlock {
                label: "Total in top categories".to_string(),
                value: format_cents(total_cents),
                detail: Some(format!("{} categories", rows.len())),
                tone: Some("neutral".to_string()),
            },
        ],
    };

    Ok(Some(AgentAnswer {
        prose,
        reasoning: "Deterministic fallback queried current-month negative transactions, grouped them by category, and ranked categories by total spend.".to_string(),
        plan: Vec::new(),
        trace: vec!["Called tool: get_top_spending_categories".to_string()],
        changes: Vec::new(),
        action_label: None,
        action_path: None,
        bundle_id: None,
        assumptions: vec![
            "Current month is calculated from the local database clock.".to_string(),
            "Expenses are treated as negative transaction amounts.".to_string(),
            "Uncategorized transactions are grouped as Uncategorized.".to_string(),
        ],
        data_sources: vec![
            "Local transactions table".to_string(),
            "Local categories table".to_string(),
        ],
        missing_data: Vec::new(),
        alternatives: Vec::new(),
        follow_up_questions: vec![
            "Show the largest individual transactions in this category.".to_string(),
            "Compare this category against last month.".to_string(),
            "Help me reduce this category next month.".to_string(),
        ],
        response_blocks: vec![metrics, table, chart],
    }))
}

struct SpendingCategoryRow {
    category: String,
    amount_cents: i64,
    transaction_count: i64,
}

fn top_spending_categories_this_month(
    conn: &mut rusqlite::Connection,
) -> rusqlite::Result<Vec<SpendingCategoryRow>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(c.label, 'Uncategorized') AS category,
                CAST(SUM(-t.amount_cents) AS INTEGER) AS spent_cents,
                COUNT(*) AS txn_count
         FROM transactions t
         LEFT JOIN categories c ON c.id = t.category_id
         WHERE t.amount_cents < 0
           AND COALESCE(t.pending, 0) = 0
           AND date(t.posted_at) >= date('now', 'start of month')
           AND date(t.posted_at) < date('now', 'start of month', '+1 month')
         GROUP BY COALESCE(c.label, 'Uncategorized')
         HAVING spent_cents > 0
         ORDER BY spent_cents DESC
         LIMIT 5",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SpendingCategoryRow {
            category: row.get(0)?,
            amount_cents: row.get(1)?,
            transaction_count: row.get(2)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn format_cents(cents: i64) -> String {
    let value = cents as f64 / 100.0;
    if value.fract().abs() < 0.005 {
        format!("${value:.0}")
    } else {
        format!("${value:.2}")
    }
}

/// Build the final question string by prepending conversation history as context.
fn build_question_with_history(text: &str, history: &[ChatHistoryEntry]) -> String {
    if history.is_empty() {
        return text.to_string();
    }

    // Keep the last 10 turns to avoid bloating the context window.
    let relevant: Vec<&ChatHistoryEntry> = history
        .iter()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let history_text: Vec<String> = relevant
        .iter()
        .map(|e| {
            let role = if e.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            format!(
                "{role}: {}",
                e.content.chars().take(500).collect::<String>()
            )
        })
        .collect();

    format!(
        "--- Prior conversation context ---\n{}\n--- End context ---\n\nCurrent question: {text}",
        history_text.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_recategorization_preview_builds_a_block_from_recategorize_bulk_draft_actions() {
        let draft_actions = vec![finsight_agent::reasoning::messages::AgentDraftAction {
            action_kind: "recategorize_bulk".to_string(),
            payload_json: serde_json::json!({
                "assignments": [
                    { "transactionId": "t1", "categoryId": "c1", "categoryLabel": "Groceries", "merchant": "Trader Joe's", "confidence": 0.99 },
                    { "transactionId": "t2", "categoryId": "c2", "categoryLabel": "Transport", "merchant": "Shell", "confidence": 0.97 },
                ]
            })
            .to_string(),
            rationale: "Recategorize 2 uncategorized transactions.".to_string(),
            confidence: 0.98,
        }];

        let block = synthesize_recategorization_preview(&draft_actions, "bundle-xyz");
        assert!(block.is_some());
        let AgentResponseBlock::RecategorizationPreview(preview) = block.unwrap() else {
            panic!("expected a RecategorizationPreview block");
        };
        assert_eq!(preview.bundle_id, "bundle-xyz");
        assert_eq!(preview.count, 2);
        assert_eq!(preview.rows.len(), 2);
        assert_eq!(preview.rows[0].merchant, "Trader Joe's");
        assert_eq!(preview.more, 0);
    }

    fn transaction_table_answer(query: Option<AgentTxnSearchQuery>) -> AgentAnswer {
        let mut answer = AgentAnswer {
            prose: String::new(),
            reasoning: String::new(),
            plan: Vec::new(),
            trace: Vec::new(),
            changes: Vec::new(),
            action_label: None,
            action_path: None,
            bundle_id: None,
            assumptions: Vec::new(),
            data_sources: Vec::new(),
            missing_data: Vec::new(),
            alternatives: Vec::new(),
            follow_up_questions: Vec::new(),
            response_blocks: vec![AgentResponseBlock::TransactionTable(
                crate::commands::agent::AgentTransactionTableBlock {
                    count: 1,
                    total_cents: 9000,
                    rows: vec![crate::commands::agent::AgentTxRow {
                        date: "2026-05-03".into(),
                        merchant: "Costco".into(),
                        category_key: "Groceries".into(),
                        amount_cents: 9000,
                        flag: None,
                    }],
                    more: 0,
                    query,
                },
            )],
        };
        answer.response_blocks.shrink_to_fit();
        answer
    }

    #[test]
    fn attaches_captured_search_query_to_transaction_table_block() {
        let mut answer = transaction_table_answer(None);
        let captured = Mutex::new(Some(serde_json::json!({
            "account": "amex", "min_amount_cents": 6000, "direction": "expense", "start_date": "2026-05-01"
        })));
        attach_search_query_to_transaction_tables(&mut answer, &captured);
        let AgentResponseBlock::TransactionTable(t) = &answer.response_blocks[0] else {
            panic!("expected a TransactionTable block");
        };
        let q = t.query.as_ref().expect("query should be attached");
        assert_eq!(q.account.as_deref(), Some("amex"));
        assert_eq!(q.min_amount_cents, Some(6000));
        assert_eq!(q.direction.as_deref(), Some("expense"));
        assert_eq!(q.start_date.as_deref(), Some("2026-05-01"));
        assert_eq!(q.merchant, None);
    }

    #[test]
    fn does_not_overwrite_an_existing_transaction_table_query_or_invent_one() {
        // No captured args → query stays None (no export offered).
        let mut answer = transaction_table_answer(None);
        attach_search_query_to_transaction_tables(&mut answer, &Mutex::new(None));
        let AgentResponseBlock::TransactionTable(t) = &answer.response_blocks[0] else {
            panic!();
        };
        assert!(t.query.is_none());

        // "any" direction is dropped, matching the tool's own arg handling.
        let mut answer = transaction_table_answer(None);
        attach_search_query_to_transaction_tables(
            &mut answer,
            &Mutex::new(Some(serde_json::json!({ "direction": "any" }))),
        );
        let AgentResponseBlock::TransactionTable(t) = &answer.response_blocks[0] else {
            panic!();
        };
        assert_eq!(t.query.as_ref().unwrap().direction, None);

        // A block that already carries a query is left untouched.
        let mut answer = transaction_table_answer(Some(AgentTxnSearchQuery {
            merchant: Some("Costco".into()),
            account: None,
            start_date: None,
            end_date: None,
            min_amount_cents: None,
            direction: None,
        }));
        attach_search_query_to_transaction_tables(
            &mut answer,
            &Mutex::new(Some(serde_json::json!({ "account": "amex" }))),
        );
        let AgentResponseBlock::TransactionTable(t) = &answer.response_blocks[0] else {
            panic!();
        };
        assert_eq!(t.query.as_ref().unwrap().merchant.as_deref(), Some("Costco"));
        assert_eq!(t.query.as_ref().unwrap().account, None);
    }

    #[test]
    fn synthesize_recategorization_preview_caps_rows_at_five_and_reports_the_remainder() {
        let assignments: Vec<serde_json::Value> = (0..7)
            .map(|i| {
                serde_json::json!({
                    "transactionId": format!("t{i}"),
                    "categoryId": format!("c{i}"),
                    "categoryLabel": "Groceries",
                    "merchant": format!("Merchant {i}"),
                    "confidence": 0.9
                })
            })
            .collect();
        let draft_actions = vec![finsight_agent::reasoning::messages::AgentDraftAction {
            action_kind: "recategorize_bulk".to_string(),
            payload_json: serde_json::json!({ "assignments": assignments }).to_string(),
            rationale: "Recategorize 7 uncategorized transactions.".to_string(),
            confidence: 0.9,
        }];

        let AgentResponseBlock::RecategorizationPreview(preview) =
            synthesize_recategorization_preview(&draft_actions, "bundle-xyz").unwrap()
        else {
            panic!("expected a RecategorizationPreview block");
        };
        assert_eq!(preview.count, 7);
        assert_eq!(preview.rows.len(), 5);
        assert_eq!(preview.more, 2);
    }

    #[test]
    fn synthesize_recategorization_preview_returns_none_without_a_recategorize_bulk_action() {
        let draft_actions = vec![finsight_agent::reasoning::messages::AgentDraftAction {
            action_kind: "set_budget".to_string(),
            payload_json: "{}".to_string(),
            rationale: "unrelated".to_string(),
            confidence: 0.9,
        }];
        assert!(synthesize_recategorization_preview(&draft_actions, "bundle-xyz").is_none());
    }

    #[test]
    fn small_table_is_within_artifact_bounds() {
        let block = AgentResponseBlock::Table(AgentTableBlock {
            title: Some("Top spending".into()),
            columns: vec!["Category".into(), "Spent".into()],
            rows: vec![vec!["Dining".into(), "$8,370".into()]],
        });
        assert!(response_block_within_artifact_bounds(&block));
    }

    #[test]
    fn oversized_table_is_rejected() {
        let block = AgentResponseBlock::Table(AgentTableBlock {
            title: None,
            columns: vec!["a".into(), "b".into()],
            rows: (0..ARTIFACT_MAX_TABLE_ROWS + 1)
                .map(|_| vec!["x".into(), "y".into()])
                .collect(),
        });
        assert!(!response_block_within_artifact_bounds(&block));
    }

    #[test]
    fn non_finite_chart_value_is_rejected() {
        let block = AgentResponseBlock::BarChart(AgentChartBlock {
            title: None,
            series_label: None,
            data: vec![AgentChartPoint {
                label: "NaN point".into(),
                value: f64::NAN,
            }],
        });
        assert!(!response_block_within_artifact_bounds(&block));
    }

    #[test]
    fn too_many_metrics_is_rejected() {
        let block = AgentResponseBlock::MetricGrid {
            metrics: (0..ARTIFACT_MAX_METRICS + 1)
                .map(|_| AgentMetricBlock {
                    label: "l".into(),
                    value: "v".into(),
                    detail: None,
                    tone: None,
                })
                .collect(),
        };
        assert!(!response_block_within_artifact_bounds(&block));
    }
}
