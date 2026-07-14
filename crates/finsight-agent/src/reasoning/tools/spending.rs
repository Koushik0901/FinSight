use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use finsight_core::spending::baseline::{self, Baseline};
use finsight_core::spending::decompose::{decompose, Filter};
use finsight_core::spending::Window;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn explain_spending_change() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "explain_spending_change"
        }
        fn description(&self) -> &str {
            "Explain WHAT CHANGED in a month's spending versus the user's normal — the ranked drivers of the difference, each tagged with a mechanism (new / price_up / frequency_up / stopped) and a persistence (recurring / one_off / emerging). Use for 'why was <month> so high', 'what's new this month vs my usual', 'what doubled', 'how much of the increase will recur' (read persistence_subtotals), and 'compare <month> to <other month>'. `period` is a YYYY-MM month. By default it compares against the trailing-12-month baseline; pass `reference` (YYYY-MM) to compare two specific months. Every number is precomputed — quote the *_display strings and the persistence_subtotals; do not add or divide amounts yourself."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "period": {"type":"string","description":"Target month, YYYY-MM (e.g. 2026-05)."},
                "reference": {"type":"string","description":"Optional comparison month YYYY-MM. Omit to compare against the trailing-12-month normal."},
                "filter": {"type":"string","enum":["all","new","elevated"],"default":"all","description":"'new' = only merchants absent from the baseline; 'elevated' = only merchants at least min_ratio× their usual."},
                "min_ratio": {"type":"number","default":2.0,"description":"Threshold for filter='elevated'."},
                "limit": {"type":"integer","default":12}
            },"required":["period"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let period = args["period"].as_str().unwrap_or("").to_string();
            if period.len() < 7 {
                return Ok(json!({"error":"bad_period","note":"period must be YYYY-MM"}));
            }
            let filter = match args["filter"].as_str().unwrap_or("all") {
                "new" => Filter::New,
                "elevated" => Filter::Elevated,
                _ => Filter::All,
            };
            let min_ratio = args["min_ratio"].as_f64().unwrap_or(2.0);
            let limit = args["limit"].as_i64().unwrap_or(12).clamp(1, 50) as usize;

            // Reference: an explicit month, else the trailing 12 months ending
            // the month BEFORE `period` (so the target isn't in its own baseline).
            let reference: Baseline = match args["reference"].as_str() {
                Some(rm) if rm.len() >= 7 => {
                    let (ry, rmn) = finsight_core::spending::parse_ym(rm);
                    let end = if rmn == 12 {
                        format!("{:04}-01", ry + 1)
                    } else {
                        format!("{ry:04}-{:02}", rmn + 1)
                    };
                    baseline::compute(ctx.conn, rm, &end).map_err(|e| anyhow::anyhow!(e.to_string()))?
                }
                _ => finsight_core::spending::baseline::trailing(ctx.conn, &period, 12)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            };

            let target = Window::for_month(&period);
            let out = decompose(ctx.conn, &target, &reference, filter, min_ratio, limit)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(serde_json::to_value(out)?)
        }
    }
    Arc::new(T)
}

pub fn classify_spending_period() -> std::sync::Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "classify_spending_period"
        }
        fn description(&self) -> &str {
            "Judge whether a month is normal, an episodic one-off spike, or a sustained new regime, versus the user's own trailing history. Use for 'was last month a blip or my new normal?'. `period` is YYYY-MM. Returns the class plus evidence (the month's total, the normal median, the upper band, and how many recent months were also elevated) — all precomputed; quote the *_display values, don't recompute."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "period":{"type":"string","description":"Month to judge, YYYY-MM."}
            },"required":["period"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let period = args["period"].as_str().unwrap_or("");
            if period.len() < 7 {
                return Ok(json!({"error":"bad_period","note":"period must be YYYY-MM"}));
            }
            let a = finsight_core::spending::classify::classify_spending_period(ctx.conn, period)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(serde_json::to_value(a)?)
        }
    }
    std::sync::Arc::new(T)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::messages::{AgentChange, AgentDraftAction};
    use finsight_core::{db::run_migrations, keychain, Db};
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        {
            let conn = db.get().unwrap();
            conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Credit','Card','USD','#fff',datetime('now'))", []).unwrap();
        }
        (dir, db)
    }

    fn ins(conn: &Connection, ym: &str, cents: i64, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,0,'cleared',datetime('now'))",
            rusqlite::params![format!("{ym}-15T12:00:00Z"), cents, merchant],
        ).unwrap();
    }

    #[test]
    fn tool_reports_new_flight_as_top_driver() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-05", -60_000, "FLAIR AIRLINES  BURNABY, BC");

        let mut changes: Vec<AgentChange> = Vec::new();
        let mut drafts: Vec<AgentDraftAction> = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = explain_spending_change().execute(&mut ctx, json!({"period":"2026-05"})).unwrap();

        let drivers = out["drivers"].as_array().unwrap();
        assert_eq!(drivers[0]["display"], "FLAIR AIRLINES");
        assert_eq!(drivers[0]["mechanism"], "new");
    }

    #[test]
    fn classify_tool_flags_episodic_spike() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-01", -900_000, "FLAIR AIRLINES  BURNABY, BC");
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = classify_spending_period().execute(&mut ctx, json!({"period":"2026-01"})).unwrap();
        assert_eq!(out["class"], "episodic_spike");
    }
}
