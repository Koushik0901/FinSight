//! Transport-agnostic command surface. Each command is a plain
//! `async fn(&ApiState, args) -> AppResult<T>` with no Tauri types, so it can be
//! driven equally by the Tauri command wrappers and by finsight-server's HTTP
//! dispatcher. Command modules are added here as they are extracted from
//! finsight-app.

pub mod accounts;
pub mod agent;
pub mod assets;
pub mod budget;
pub mod categories;
pub mod copilot;
pub mod data_health;
pub mod household;
pub mod import;
pub mod inbox;
pub mod insights;
pub mod investments;
pub mod journey;
pub mod meta;
pub mod metrics;
pub mod onboarding;
pub mod planned_transactions;
pub mod recipes;
pub mod recurring;
pub mod reports;
pub mod scenarios;
pub mod settings;
pub mod simplefin;
pub mod spending;
pub mod transactions;
