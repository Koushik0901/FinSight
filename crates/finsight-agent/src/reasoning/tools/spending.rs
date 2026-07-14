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
            "Explain WHAT CHANGED in a month's spending versus the user's normal — the ranked drivers of the difference, each tagged with a mechanism (new / price_up / frequency_up / stopped) and a persistence (recurring / one_off / emerging). Use for 'why was <month> so high', 'what's new this month vs my usual', 'what doubled', 'how much of the increase will recur' (read persistence_subtotals), and 'compare <month> to <other month>'. `period` is a YYYY-MM month. By default it compares against the trailing-12-month baseline; pass `reference` (YYYY-MM) to compare two specific months. Every number is precomputed — quote the *_display strings and the persistence_subtotals; do not add or divide amounts yourself. A driver with a non-empty user_verdict is one the user has accepted (expected/investment) or marked one_off — never present it as a recurring lever, whatever its persistence."
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

pub fn annotate_spending_driver() -> std::sync::Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "annotate_spending_driver"
        }
        fn description(&self) -> &str {
            "Remember the user's verdict on a spending driver so it stops showing as a recurring lever everywhere. Pass the `merchant_key` exactly as returned by explain_spending_change. `verdict`: one_off (a one-time thing), expected (a known/accepted cost), investment (spending the user considers an investment), or reset (forget a prior verdict). This WRITES immediately and is remembered across sessions. Only call it when the user has actually told you their verdict."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "merchant_key":{"type":"string","description":"canonical merchant key from explain_spending_change output"},
                "verdict":{"type":"string","enum":["one_off","expected","investment","reset"]},
                "note":{"type":"string"}
            },"required":["merchant_key","verdict"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            use crate::reasoning::messages::AgentChange;
            let key = args["merchant_key"].as_str().unwrap_or("").trim();
            let verdict = args["verdict"].as_str().unwrap_or("");
            if key.is_empty() {
                return Ok(json!({"error":"missing_merchant_key"}));
            }
            let note = args["note"].as_str();
            if verdict == "reset" {
                finsight_core::spending::annotate::clear_annotation(ctx.conn, key)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            } else if finsight_core::spending::annotate::VERDICTS.contains(&verdict) {
                let known = finsight_core::spending::annotate::known_driver_keys(ctx.conn)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                if !known.contains(key) {
                    return Ok(json!({"saved": false, "error": "unknown_merchant_key", "note": "No spending driver matches that merchant_key. Pass the exact merchant_key from explain_spending_change output."}));
                }
                finsight_core::spending::annotate::set_annotation(ctx.conn, key, verdict, note)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            } else {
                return Ok(json!({"error":"bad_verdict","note":"verdict must be one_off, expected, investment, or reset"}));
            }
            ctx.changes.push(AgentChange {
                kind: "spending_annotation".to_string(),
                description: format!("Marked '{key}' as {verdict}"),
            });
            Ok(json!({"saved": true, "merchant_key": key, "verdict": verdict}))
        }
    }
    std::sync::Arc::new(T)
}

pub fn plan_spending_reduction() -> std::sync::Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "plan_spending_reduction"
        }
        fn description(&self) -> &str {
            "Build an HONEST path back toward a spending target. Given `period` (YYYY-MM, the elevated month; omit to use the most recent month) and optional `target_monthly_cents`, it separates one-off spend (self_correcting_cents — falls off on its own, no action) from the recurring 'levers' you can trim (recoverable_recurring_cents + the `levers` list), projects where trimming lands you (projected_after_levers_cents), and sets structural_gap_cents when the target is BELOW what trimming can reach — meaning the rest is a structural floor, not more cuts. Read the `note`. Use for 'how do I get back to $X' / 'how do I cut my spending'. Every number is precomputed — quote the *_display values and never claim a target is reachable when structural_gap_cents is set."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "period":{"type":"string","description":"Elevated month YYYY-MM. Omit to use the most recent month with activity."},
                "target_monthly_cents":{"type":"integer","description":"Optional monthly spend goal in cents (e.g. 150000 for $1,500/mo)."}
            }})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let period = match args["period"].as_str() {
                Some(p) if p.len() >= 7 => p.to_string(),
                _ => match finsight_core::spending::baseline::latest_activity_month(ctx.conn)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                {
                    Some(ym) => ym,
                    None => return Ok(json!({"error":"no_data","note":"No spending activity to plan from."})),
                },
            };
            let target = args["target_monthly_cents"].as_i64();
            let plan = finsight_core::spending::plan::plan_spending_reduction(ctx.conn, &period, target)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mut v = serde_json::to_value(plan)?;
            v["period"] = json!(period);
            Ok(v)
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

    #[test]
    fn annotate_tool_writes_a_sticky_verdict() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        ins(&conn, "2026-01", -50_000, "FLAIR AIRLINES  BURNABY, BC");
        let key = finsight_core::merchant::canonical_merchant_key("FLAIR AIRLINES  BURNABY, BC");
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        {
            let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
            let out = annotate_spending_driver()
                .execute(&mut ctx, json!({"merchant_key": key, "verdict": "one_off"}))
                .unwrap();
            assert_eq!(out["saved"], true);
        }
        assert_eq!(
            finsight_core::spending::annotate::annotations(&conn).unwrap().get(&key).unwrap(),
            "one_off"
        );
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn annotate_tool_rejects_unknown_merchant_key() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = annotate_spending_driver()
            .execute(&mut ctx, json!({"merchant_key": "nonexistent vendor", "verdict": "one_off"}))
            .unwrap();
        assert_eq!(out["saved"], false);
        assert_eq!(out["error"], "unknown_merchant_key");
    }

    #[test]
    fn plan_tool_flags_structural_target() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -200_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-01", -250_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-01", -90_000, "FLAIR AIRLINES  BURNABY, BC");
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = plan_spending_reduction()
            .execute(&mut ctx, json!({"period":"2026-01","target_monthly_cents":150_000}))
            .unwrap();
        assert_eq!(out["structural_gap_cents"], 50_000);
        assert_eq!(out["self_correcting_cents"], 90_000);
        assert_eq!(out["period"], "2026-01");
    }
}
