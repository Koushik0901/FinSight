//! Transport-agnostic command surface. Each command is a plain
//! `async fn(&ApiState, args) -> AppResult<T>` with no Tauri types, so it can be
//! driven equally by the Tauri command wrappers and by finsight-server's HTTP
//! dispatcher. Command modules are added here as they are extracted from
//! finsight-app.

pub mod accounts;
pub mod categories;
pub mod investments;
pub mod meta;
pub mod onboarding;
pub mod transactions;
