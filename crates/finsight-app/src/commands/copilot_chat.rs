//! Copilot chat commands — persistent threaded conversations with streaming.
//!
//! Each command stores messages in the `conversations` / `conversation_messages`
//! SQLite tables (V029 migration). Streaming is simulated: the full answer is
//! produced by the reasoning engine, then emitted word-by-word via `copilot-token`
//! Tauri events so the frontend sees a natural typing effect.

use crate::commands::agent::{
    build_toolset, enrich_agent_answer, is_usable_tool_answer, planner_answer_to_agent_answer,
    reasoning_result_to_agent_answer, validate_finance_answer, AgentAnswer, AgentResponseBlock,
};
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_agent::{planning, reasoning::engine::ReasoningEngine};
use finsight_core::models::{ConversationMessage, ConversationSummary};
use finsight_core::repos::{conversations, run};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;
use std::sync::Arc;
use std::time::Instant;
use tauri::Emitter;

// ── Public types emitted as Tauri events ────────────────────────────────────

#[derive(Debug, Serialize, Clone, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CopilotStreamFrame {
    Text {
        conversation_id: String,
        run_id: String,
        delta: String,
    },
    Reasoning {
        conversation_id: String,
        run_id: String,
        text: String,
    },
    ToolCallStart {
        conversation_id: String,
        run_id: String,
        tool_call_id: String,
        tool_name: String,
        args: Value,
    },
    ToolCallResult {
        conversation_id: String,
        run_id: String,
        tool_call_id: String,
        result: Value,
        is_error: bool,
    },
    ResponseBlock {
        conversation_id: String,
        run_id: String,
        block_id: String,
        block: AgentResponseBlock,
    },
    Source {
        conversation_id: String,
        run_id: String,
        source_id: String,
        title: String,
    },
    Usage {
        conversation_id: String,
        run_id: String,
        provider_id: String,
        model_id: String,
        elapsed_ms: u64,
        tool_count: u32,
    },
    Done {
        conversation_id: String,
        run_id: String,
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
    },
    Error {
        conversation_id: String,
        run_id: String,
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
/// 3. Streams the answer word-by-word via `copilot-token` events.
/// 4. Persists the assistant message and emits `copilot-done`.
/// 5. Auto-generates a title for new conversations after the first message.
#[tauri::command]
#[specta::specta]
pub async fn stream_copilot_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    conversation_id: String,
    run_id: String,
    text: String,
    history: Vec<ChatHistoryEntry>,
    source_message_id: Option<String>,
) -> AppResult<String> {
    let started_at = Instant::now();
    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        return Err(AppError::new(
            "no_provider",
            "Configure an AI provider in Settings → Agent to use this feature.",
        ));
    };

    let db = (*state.db).clone();
    let conv_id = conversation_id.clone();

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
    if let Some(source_id) = source_message_id.clone() {
        let cid = conv_id.clone();
        let txt = text.clone();
        let parts = user_parts_json.clone();
        run(&db, move |conn| {
            conversations::update_user_message(conn, &source_id, &txt, Some(&parts))?;
            conversations::delete_messages_after(conn, &cid, &source_id)?;
            Ok::<_, finsight_core::CoreError>(())
        })
        .await
        .map_err(AppError::from)?;
    } else {
        let cid = conv_id.clone();
        let txt = text.clone();
        let parts = user_parts_json.clone();
        run(&db, move |conn| {
            conversations::insert_message(conn, &cid, "user", &txt, None, None, None, Some(&parts))
                .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
        })
        .await
        .map_err(AppError::from)?;
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
    let provider_clone = Arc::clone(&provider);
    let question_for_engine = enriched_question.clone();
    let tool_result = run(&db, move |conn| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                finsight_core::CoreError::InvalidState(format!("Failed to create runtime: {e}"))
            })?;
        rt.block_on(ReasoningEngine::run(
            conn,
            &question_for_engine,
            &tools,
            provider_clone,
            10,
        ))
        .map_err(|e| finsight_core::CoreError::InvalidState(format!("Reasoning engine error: {e}")))
    })
    .await;

    // 5. Build AgentAnswer from result
    let mut answer: AgentAnswer = match tool_result {
        Ok(result) if is_usable_tool_answer(&result) => {
            let draft_actions = result.draft_actions.clone();
            let question_for_db = enriched_question.clone();
            let content_for_db = result.content.clone();
            let reasoning_for_db = if result.reasoning.is_empty() {
                "Tool-driven financial analysis".to_string()
            } else {
                result.reasoning.clone()
            };
            let provider_id = provider.provider_id().to_string();
            let model_id = provider.model_id().to_string();
            let bundle_id = run(&db, move |conn| {
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
            .map_err(AppError::from)?;

            let mut answer = reasoning_result_to_agent_answer(result, Some(bundle_id));
            validate_finance_answer(&enriched_question, &mut answer);
            enrich_agent_answer(&mut answer);
            answer
        }
        Ok(result) => {
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
                let mut answer = reasoning_result_to_agent_answer(result, None);
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
            } else {
                return Err(AppError::new("agent.reasoning", tool_err.to_string()));
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
            return Err(AppError::new(
                "agent.empty_response",
                "Copilot finished without a text response. Check the configured AI provider/model in Settings -> Agent, then try again.",
            ));
        }
    }

    let provider_id = provider.provider_id().to_string();
    let model_id = provider.model_id().to_string();
    let tool_names = tool_names_from_trace(&answer.trace);

    // 6. Emit rich assistant-ui parts before the final text stream.
    if !answer.reasoning.trim().is_empty() {
        emit_copilot_frame(
            &app,
            CopilotStreamFrame::Reasoning {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                text: answer.reasoning.clone(),
            },
        );
    }

    for (i, tool_name) in tool_names.iter().enumerate() {
        let tool_call_id = format!("tool-{i}");
        emit_copilot_frame(
            &app,
            CopilotStreamFrame::ToolCallStart {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                args: json!({}),
            },
        );
        emit_copilot_frame(
            &app,
            CopilotStreamFrame::ToolCallResult {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                tool_call_id,
                result: json!({
                    "ok": true,
                    "summary": answer.trace.get(i).cloned().unwrap_or_else(|| tool_name.clone()),
                }),
                is_error: false,
            },
        );
    }

    for (i, block) in answer.response_blocks.iter().cloned().enumerate() {
        emit_copilot_frame(
            &app,
            CopilotStreamFrame::ResponseBlock {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                block_id: format!("block-{i}"),
                block,
            },
        );
    }

    for (i, title) in answer.data_sources.iter().cloned().enumerate() {
        emit_copilot_frame(
            &app,
            CopilotStreamFrame::Source {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                source_id: format!("source-{i}"),
                title,
            },
        );
    }

    // 7. Simulated text streaming: emit prose word-by-word at ~25 ms per word.
    let words: Vec<&str> = answer.prose.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        let delta = if i + 1 < words.len() {
            format!("{} ", word)
        } else {
            word.to_string()
        };
        emit_copilot_frame(
            &app,
            CopilotStreamFrame::Text {
                conversation_id: conv_id.clone(),
                run_id: run_id.clone(),
                delta,
            },
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
    }

    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    emit_copilot_frame(
        &app,
        CopilotStreamFrame::Usage {
            conversation_id: conv_id.clone(),
            run_id: run_id.clone(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
            elapsed_ms,
            tool_count: tool_names.len() as u32,
        },
    );

    // 8. Persist assistant message
    let assistant_prose = answer.prose.clone();
    let bundle_id_for_db = answer.bundle_id.clone();
    let trace_json = serde_json::to_string(&answer.trace).unwrap_or_default();
    let parts_json = assistant_parts_json(&answer);
    let asst_msg = {
        let cid = conv_id.clone();
        let parts = parts_json.clone();
        run(&db, move |conn| {
            conversations::insert_message(
                conn,
                &cid,
                "assistant",
                &assistant_prose,
                Some(trace_json.as_str()),
                bundle_id_for_db.as_deref(),
                None,
                Some(&parts),
            )
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
        })
        .await
        .map_err(AppError::from)?
    };
    let asst_msg_id = asst_msg.id.clone();

    // 9. Emit completion frame.
    emit_copilot_frame(
        &app,
        CopilotStreamFrame::Done {
            conversation_id: conv_id.clone(),
            run_id: run_id.clone(),
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

/// List all conversations for the sidebar, most-recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_conversations(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<ConversationSummary>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        conversations::list_conversations(conn)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
}

/// Fetch all messages for a given conversation, ordered oldest-first.
#[tauri::command]
#[specta::specta]
pub async fn get_conversation_messages(
    state: tauri::State<'_, AppState>,
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

/// Delete a conversation and all its messages.
#[tauri::command]
#[specta::specta]
pub async fn delete_conversation(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        conversations::delete_conversation(conn, &id)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
}

/// Create a new empty conversation and return its ID.
#[tauri::command]
#[specta::specta]
pub async fn create_conversation(state: tauri::State<'_, AppState>) -> AppResult<String> {
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

/// Edit a persisted user message and remove later turns so assistant-ui reload/edit
/// operations have durable backend semantics.
#[tauri::command]
#[specta::specta]
pub async fn edit_conversation_user_message(
    state: tauri::State<'_, AppState>,
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

/// Delete messages after a selected turn. The frontend then starts a fresh run
/// from the remaining thread history.
#[tauri::command]
#[specta::specta]
pub async fn delete_conversation_messages_after(
    state: tauri::State<'_, AppState>,
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

fn emit_copilot_frame(app: &tauri::AppHandle, frame: CopilotStreamFrame) {
    let _ = app.emit("copilot-stream-frame", frame);
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

    for (i, block) in answer.response_blocks.iter().enumerate() {
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
