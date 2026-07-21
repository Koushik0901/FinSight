use crate::commands::TauriFrameSink;
use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{ConversationMessage, ConversationSummary};
use std::sync::Arc;

// Types + the tauri-free command bodies live in finsight-api now; re-exported
// so existing imports of `finsight_bindings::commands::copilot_chat::*` (lib.rs,
// tests) keep resolving.
pub use finsight_api::commands::copilot_chat::{
    ChatHistoryEntry, CopilotStreamFrame, EditConversationMessageInput,
};

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
    let sink: Arc<dyn finsight_api::sink::FrameSink> = Arc::new(TauriFrameSink(app));
    finsight_api::commands::copilot_chat::stream_copilot_message(
        &state.api,
        sink,
        conversation_id,
        run_id,
        text,
        history,
        source_message_id,
    )
    .await
}

/// List all conversations for the sidebar, most-recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_conversations(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<ConversationSummary>> {
    finsight_api::commands::copilot_chat::list_conversations(&state.api).await
}

/// Fetch all messages for a given conversation, ordered oldest-first.
#[tauri::command]
#[specta::specta]
pub async fn get_conversation_messages(
    state: tauri::State<'_, AppState>,
    conversation_id: String,
) -> AppResult<Vec<ConversationMessage>> {
    finsight_api::commands::copilot_chat::get_conversation_messages(&state.api, conversation_id)
        .await
}

/// Delete a conversation and all its messages.
#[tauri::command]
#[specta::specta]
pub async fn delete_conversation(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::copilot_chat::delete_conversation(&state.api, id).await
}

/// Create a new empty conversation and return its ID.
#[tauri::command]
#[specta::specta]
pub async fn create_conversation(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::copilot_chat::create_conversation(&state.api).await
}

/// Edit a persisted user message and remove later turns so assistant-ui reload/edit
/// operations have durable backend semantics.
#[tauri::command]
#[specta::specta]
pub async fn edit_conversation_user_message(
    state: tauri::State<'_, AppState>,
    input: EditConversationMessageInput,
) -> AppResult<()> {
    finsight_api::commands::copilot_chat::edit_conversation_user_message(&state.api, input).await
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
    finsight_api::commands::copilot_chat::delete_conversation_messages_after(
        &state.api,
        conversation_id,
        message_id,
    )
    .await
}
