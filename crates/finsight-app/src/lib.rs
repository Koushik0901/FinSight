//! FinSight Tauri app — command surface + lifecycle.

pub mod commands;
pub mod error;

use finsight_core::Db;
use std::sync::Arc;

pub struct AppState {
    pub db: Arc<Db>,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { db: Arc::new(db) }
    }
}
