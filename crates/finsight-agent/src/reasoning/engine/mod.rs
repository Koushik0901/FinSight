use crate::reasoning::messages::{AgentChange, AssistantTurn, ChatMessage, ReasoningResult};
use crate::reasoning::tools::{ToolContext, ToolSet};
use crate::CompletionProvider;
use anyhow::Result;
use std::sync::Arc;

pub struct ReasoningEngine;

impl ReasoningEngine {
    pub async fn run(
        conn: &mut rusqlite::Connection,
        question: &str,
        tools: &ToolSet,
        provider: Arc<dyn CompletionProvider>,
        max_iterations: usize,
    ) -> Result<ReasoningResult> {
        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::System { content: Self::build_system_prompt(tools) },
            ChatMessage::User { content: question.to_string() },
        ];
        let mut trace: Vec<String> = Vec::new();
        let mut changes: Vec<AgentChange> = Vec::new();

        for _ in 0..max_iterations {
            let turn = provider.complete_tool_turn(&messages, &tools.definitions()).await?;

            match turn {
                AssistantTurn::ToolCalls(calls) => {
                    let mut tool_result_msgs = Vec::new();
                    for call in &calls {
                        trace.push(format!("Called tool: {}", call.name));
                        let mut ctx = ToolContext { conn, changes: &mut changes };
                        let result = tools.execute(&call.name, &mut ctx, call.arguments.clone())?;
                        tool_result_msgs.push(ChatMessage::Tool {
                            tool_call_id: call.id.clone(),
                            content: result.to_string(),
                        });
                    }
                    messages.push(ChatMessage::Assistant {
                        content: None,
                        tool_calls: calls,
                    });
                    for msg in tool_result_msgs {
                        messages.push(msg);
                    }
                }
                AssistantTurn::FinalAnswer { content, reasoning } => {
                    return Ok(ReasoningResult {
                        content,
                        reasoning,
                        trace,
                        changes,
                    });
                }
            }
        }

        Ok(ReasoningResult {
            content: "I analyzed your finances but ran out of reasoning steps. Here's what I found so far.".to_string(),
            reasoning: "The question was too complex for the iteration limit.".to_string(),
            trace,
            changes,
        })
    }

    fn build_system_prompt(tools: &ToolSet) -> String {
        let tool_defs = tools.definitions();
        let tool_list: String = tool_defs.iter().map(|t| {
            format!("- {}: {} Parameters: {}", t.name, t.description, t.parameters)
        }).collect::<Vec<_>>().join("\n");

        format!(
            "You are a personal financial analyst for a local-first personal finance app.\n\
             You have access to the following tools:\n{}\n\n\
             When you need data, call the appropriate tool(s).\n\
             When you have enough information to answer, provide your final answer with reasoning.\n\
             Be specific with numbers. Explain your reasoning clearly.\n\
             Autonomous actions (update_goal_monthly, create_planned_transaction) are allowed.\n\
             Respond with either tool calls or a final answer.", tool_list
        )
    }
}

#[cfg(test)]
mod tests;
