use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub enum AssistantTurn {
    ToolCalls {
        calls: Vec<ToolCall>,
        plan: Option<Vec<String>>,
    },
    FinalAnswer {
        content: String,
        reasoning: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChange {
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDraftAction {
    pub action_kind: String,
    pub payload_json: String,
    pub rationale: String,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct ReasoningResult {
    pub content: String,
    pub reasoning: String,
    pub plan: Vec<String>,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
    pub draft_actions: Vec<AgentDraftAction>,
    pub assumptions: Vec<String>,
    pub data_sources: Vec<String>,
    pub missing_data: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub response_blocks: Vec<Value>,
    /// True when the model's final turn is a genuine answer rather than a
    /// stall: either it parsed as the JSON answer contract, or (the model
    /// sometimes answers a quick clarifying question or short decline in
    /// plain prose instead of JSON) it has substantive content once any
    /// leading `PLAN:` preamble is stripped. False only for an empty
    /// response, a bare plan with no answer after it, or content that is
    /// just intent-filler ("let me pull that data now"). Used to distinguish
    /// "correctly answered without a tool" from "failed to answer" without
    /// requiring a tool call for every valid answer.
    pub is_real_answer: bool,
}

/// Upper bounds applied to a parsed plan, since the raw text this is parsed
/// from is untrusted LLM output (this app talks to cloud model providers) —
/// caps here at the trust boundary keep an adversarial or malfunctioning
/// model response from ballooning into an unbounded `Vec<String>` that later
/// gets persisted as JSON.
const MAX_PLAN_STEPS: usize = 10;
const MAX_PLAN_STEP_CHARS: usize = 500;

/// Extracts a best-effort `PLAN:` preamble from a model's raw tool-turn text
/// content, per the system-prompt contract in `build_system_prompt`.
///
/// Looks for a line that is exactly (after trimming) `PLAN:`, followed by one
/// or more numbered lines (`1. ...`, `2. ...`, etc.), terminated by a blank
/// line or the end of the numbered run. Returns `None` if no such block is
/// found — this must never panic, only degrade gracefully, since models may
/// ignore the instruction entirely or only follow it on their first turn.
pub fn parse_plan_preamble(raw: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = raw.lines().collect();
    let plan_line_idx = lines.iter().position(|line| line.trim() == "PLAN:")?;

    let mut steps = Vec::new();
    for line in lines.iter().skip(plan_line_idx + 1) {
        if steps.len() >= MAX_PLAN_STEPS {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(after_dot) = trimmed.split_once(". ") else {
            break;
        };
        let (number, step_text) = after_dot;
        if number.trim().parse::<u32>().is_err() {
            break;
        }
        let mut step_text = step_text.trim();
        if step_text.is_empty() {
            break;
        }
        if step_text.len() > MAX_PLAN_STEP_CHARS {
            let mut end = MAX_PLAN_STEP_CHARS;
            while !step_text.is_char_boundary(end) {
                end -= 1;
            }
            step_text = &step_text[..end];
        }
        steps.push(step_text.to_string());
    }

    if steps.is_empty() {
        None
    } else {
        Some(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plan_block_with_numbered_steps() {
        let raw = "PLAN:\n1. Find the income that just landed\n2. Rank every debt by interest rate\n\n{\"tool_calls\": []}";
        let steps = parse_plan_preamble(raw).unwrap();
        assert_eq!(
            steps,
            vec![
                "Find the income that just landed".to_string(),
                "Rank every debt by interest rate".to_string(),
            ]
        );
    }

    #[test]
    fn tolerates_extra_whitespace_around_plan_marker() {
        let raw = "  PLAN:  \n1. Step one\n2. Step two\n";
        let steps = parse_plan_preamble(raw).unwrap();
        assert_eq!(steps, vec!["Step one".to_string(), "Step two".to_string()]);
    }

    #[test]
    fn returns_none_when_no_plan_marker_present() {
        assert!(parse_plan_preamble("Just some text with no plan").is_none());
    }

    #[test]
    fn returns_none_when_plan_marker_has_no_numbered_lines() {
        let raw = "PLAN:\nSome free text but not numbered\n";
        assert!(parse_plan_preamble(raw).is_none());
    }

    #[test]
    fn stops_at_blank_line_after_numbered_steps() {
        let raw = "PLAN:\n1. First\n2. Second\n\n3. Should not be included\n";
        let steps = parse_plan_preamble(raw).unwrap();
        assert_eq!(steps, vec!["First".to_string(), "Second".to_string()]);
    }

    #[test]
    fn caps_step_count_at_max_plan_steps() {
        let mut raw = String::from("PLAN:\n");
        for i in 1..=(MAX_PLAN_STEPS + 5) {
            raw.push_str(&format!("{i}. Step number {i}\n"));
        }
        let steps = parse_plan_preamble(&raw).unwrap();
        assert_eq!(steps.len(), MAX_PLAN_STEPS);
        assert_eq!(steps[0], "Step number 1");
    }

    #[test]
    fn caps_step_length_at_max_plan_step_chars() {
        let long_step = "x".repeat(MAX_PLAN_STEP_CHARS + 200);
        let raw = format!("PLAN:\n1. {long_step}\n");
        let steps = parse_plan_preamble(&raw).unwrap();
        assert_eq!(steps[0].len(), MAX_PLAN_STEP_CHARS);
    }

    #[test]
    fn truncates_multibyte_step_text_on_a_char_boundary() {
        // Each "é" is 2 bytes in UTF-8, so a naive byte-index slice at
        // MAX_PLAN_STEP_CHARS could land mid-character and panic.
        let long_step = "é".repeat(MAX_PLAN_STEP_CHARS);
        let raw = format!("PLAN:\n1. {long_step}\n");
        let steps = parse_plan_preamble(&raw).unwrap();
        assert!(steps[0].len() <= MAX_PLAN_STEP_CHARS);
        assert!(steps[0].chars().all(|c| c == 'é'));
    }
}
