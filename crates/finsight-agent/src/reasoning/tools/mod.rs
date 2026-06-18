pub mod read;
pub mod act;

use crate::reasoning::messages::ToolDefinition;
use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value>;
}

pub struct ToolContext<'a> {
    pub conn: &'a mut Connection,
    pub changes: &'a mut Vec<crate::reasoning::messages::AgentChange>,
}

pub struct ToolSet {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolSet {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| ToolDefinition {
            name: t.name().to_string(),
            description: t.description().to_string(),
            parameters: t.parameters(),
        }).collect()
    }
    pub fn execute(&self, name: &str, ctx: &mut ToolContext, args: Value) -> Result<Value> {
        let tool = self.tools.get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", name))?;
        tool.execute(ctx, args)
    }
}
