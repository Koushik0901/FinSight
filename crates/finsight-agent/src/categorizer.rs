use crate::{agent::{AgentJob, EventCallback}, CompletionProvider};
use finsight_core::Db;
use std::sync::Arc;

pub async fn run_job(
    _db: &Db,
    _job: AgentJob,
    _provider: Arc<dyn CompletionProvider>,
    _on_event: EventCallback,
) -> anyhow::Result<()> {
    // Implemented in Task 11
    Ok(())
}
