use crate::error::{CoreError, CoreResult};
use crate::models::{AgentRecipe, AgentRecipeRun};
use chrono::{Datelike, Duration, TimeZone, Timelike, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

fn map_recipe_row(r: &rusqlite::Row) -> rusqlite::Result<AgentRecipe> {
    Ok(AgentRecipe {
        id: r.get(0)?,
        title: r.get(1)?,
        description: r.get(2)?,
        recipe_kind: r.get(3)?,
        prompt_template: r.get(4)?,
        cadence: r.get(5)?,
        day_of_week: r.get(6)?,
        day_of_month: r.get(7)?,
        status: r.get(8)?,
        last_run_at: r.get(9)?,
        next_run_at: r.get(10)?,
        run_count: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

fn map_run_row(r: &rusqlite::Row) -> rusqlite::Result<AgentRecipeRun> {
    Ok(AgentRecipeRun {
        id: r.get(0)?,
        recipe_id: r.get(1)?,
        bundle_id: r.get(2)?,
        triggered_at: r.get(3)?,
        status: r.get(4)?,
        error: r.get(5)?,
        created_at: r.get(6)?,
    })
}

fn fetch_recipe(conn: &mut Connection, id: &str) -> CoreResult<AgentRecipe> {
    get(conn, id)?.ok_or_else(|| CoreError::InvalidState(format!("recipe '{id}' not found")))
}

fn increment_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

fn nine_am(year: i32, month: u32, day: u32) -> String {
    Utc.with_ymd_and_hms(year, month, day, 9, 0, 0)
        .single()
        .expect("valid trusted recipe schedule")
        .to_rfc3339()
}

fn before_nine(now: chrono::DateTime<Utc>) -> bool {
    (now.hour(), now.minute(), now.second()) < (9, 0, 0)
}

fn load_recipe_schedule(
    conn: &mut Connection,
    run_id: &str,
) -> CoreResult<(String, String, Option<i64>, Option<i64>)> {
    conn.query_row(
        "SELECT r.id, r.cadence, r.day_of_week, r.day_of_month
         FROM agent_recipe_runs rr
         JOIN agent_recipes r ON r.id = rr.recipe_id
         WHERE rr.id = ?1",
        params![run_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .map_err(Into::into)
}

fn compute_next_run(cadence: &str, day_of_week: Option<i64>, day_of_month: Option<i64>) -> String {
    let now = Utc::now();
    match cadence {
        "daily" => {
            let date = now.date_naive() + Duration::days(1);
            nine_am(date.year(), date.month(), date.day())
        }
        "weekly" => {
            let target_day = day_of_week.unwrap_or(0).clamp(0, 6) as u32;
            let today = now.weekday().num_days_from_monday();
            let mut days_ahead = (target_day + 7 - today) % 7;
            if days_ahead == 0 && !before_nine(now) {
                days_ahead = 7;
            }
            let date = now.date_naive() + Duration::days(days_ahead as i64);
            nine_am(date.year(), date.month(), date.day())
        }
        "monthly" => {
            let target_day = day_of_month.unwrap_or(1).clamp(1, 28) as u32;
            let mut year = now.year();
            let mut month = now.month();
            if now.day() > target_day || (now.day() == target_day && !before_nine(now)) {
                (year, month) = increment_month(year, month);
            }
            nine_am(year, month, target_day)
        }
        _ => (now + Duration::days(30)).to_rfc3339(),
    }
}

pub fn list(conn: &mut Connection, include_paused: bool) -> CoreResult<Vec<AgentRecipe>> {
    let mut out = Vec::new();
    if include_paused {
        let mut stmt = conn.prepare(
            "SELECT id, title, description, recipe_kind, prompt_template, cadence,
                    day_of_week, day_of_month, status, last_run_at, next_run_at,
                    run_count, created_at, updated_at
             FROM agent_recipes
             WHERE status IN ('active', 'paused')
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_recipe_row)?;
        for row in rows {
            out.push(row?);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, title, description, recipe_kind, prompt_template, cadence,
                    day_of_week, day_of_month, status, last_run_at, next_run_at,
                    run_count, created_at, updated_at
             FROM agent_recipes
             WHERE status = 'active'
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_recipe_row)?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<AgentRecipe>> {
    match conn.query_row(
        "SELECT id, title, description, recipe_kind, prompt_template, cadence,
                day_of_week, day_of_month, status, last_run_at, next_run_at,
                run_count, created_at, updated_at
         FROM agent_recipes
         WHERE id = ?1",
        params![id],
        map_recipe_row,
    ) {
        Ok(recipe) => Ok(Some(recipe)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn insert(
    conn: &mut Connection,
    title: &str,
    description: &str,
    recipe_kind: &str,
    prompt_template: &str,
    cadence: &str,
    day_of_week: Option<i64>,
    day_of_month: Option<i64>,
) -> CoreResult<AgentRecipe> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let next_run_at = compute_next_run(cadence, day_of_week, day_of_month);
    conn.execute(
        "INSERT INTO agent_recipes(
            id, title, description, recipe_kind, prompt_template, cadence,
            day_of_week, day_of_month, status, last_run_at, next_run_at,
            run_count, created_at, updated_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', NULL, ?9, 0, ?10, ?10)",
        params![
            id,
            title,
            description,
            recipe_kind,
            prompt_template,
            cadence,
            day_of_week,
            day_of_month,
            next_run_at,
            now
        ],
    )?;
    Ok(AgentRecipe {
        id,
        title: title.to_string(),
        description: description.to_string(),
        recipe_kind: recipe_kind.to_string(),
        prompt_template: prompt_template.to_string(),
        cadence: cadence.to_string(),
        day_of_week,
        day_of_month,
        status: "active".to_string(),
        last_run_at: None,
        next_run_at: Some(next_run_at),
        run_count: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn update(
    conn: &mut Connection,
    id: &str,
    title: &str,
    description: &str,
    prompt_template: &str,
    cadence: &str,
    day_of_week: Option<i64>,
    day_of_month: Option<i64>,
) -> CoreResult<AgentRecipe> {
    let now = Utc::now().to_rfc3339();
    let next_run_at = compute_next_run(cadence, day_of_week, day_of_month);
    conn.execute(
        "UPDATE agent_recipes
         SET title = ?1,
             description = ?2,
             prompt_template = ?3,
             cadence = ?4,
             day_of_week = ?5,
             day_of_month = ?6,
             next_run_at = ?7,
             updated_at = ?8
         WHERE id = ?9",
        params![
            title,
            description,
            prompt_template,
            cadence,
            day_of_week,
            day_of_month,
            next_run_at,
            now,
            id
        ],
    )?;
    fetch_recipe(conn, id)
}

pub fn set_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agent_recipes
         SET status = ?1, updated_at = ?2
         WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

pub fn list_due(conn: &mut Connection) -> CoreResult<Vec<AgentRecipe>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, recipe_kind, prompt_template, cadence,
                day_of_week, day_of_month, status, last_run_at, next_run_at,
                run_count, created_at, updated_at
         FROM agent_recipes
         WHERE status = 'active'
           AND (next_run_at IS NULL OR datetime(next_run_at) <= datetime('now'))
         ORDER BY COALESCE(next_run_at, created_at) ASC, created_at ASC",
    )?;
    let rows = stmt.query_map([], map_recipe_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn start_run(conn: &mut Connection, recipe_id: &str) -> CoreResult<AgentRecipeRun> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_recipe_runs(id, recipe_id, bundle_id, triggered_at, status, error, created_at)
         VALUES(?1, ?2, NULL, ?3, 'running', NULL, ?3)",
        params![id, recipe_id, now],
    )?;
    Ok(AgentRecipeRun {
        id,
        recipe_id: recipe_id.to_string(),
        bundle_id: None,
        triggered_at: now.clone(),
        status: "running".to_string(),
        error: None,
        created_at: now,
    })
}

pub fn complete_run(conn: &mut Connection, run_id: &str, bundle_id: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    let (recipe_id, cadence, day_of_week, day_of_month) = load_recipe_schedule(conn, run_id)?;
    let next_run_at = compute_next_run(&cadence, day_of_week, day_of_month);
    conn.execute(
        "UPDATE agent_recipe_runs
         SET status = 'completed', bundle_id = ?1, error = NULL
         WHERE id = ?2",
        params![bundle_id, run_id],
    )?;
    conn.execute(
        "UPDATE agent_recipes
         SET last_run_at = ?1,
             next_run_at = ?2,
             run_count = run_count + 1,
             updated_at = ?1
         WHERE id = ?3",
        params![now, next_run_at, recipe_id],
    )?;
    Ok(())
}

pub fn fail_run(conn: &mut Connection, run_id: &str, error: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    let (recipe_id, cadence, day_of_week, day_of_month) = load_recipe_schedule(conn, run_id)?;
    let next_run_at = compute_next_run(&cadence, day_of_week, day_of_month);
    conn.execute(
        "UPDATE agent_recipe_runs
         SET status = 'failed', error = ?1
         WHERE id = ?2",
        params![error, run_id],
    )?;
    conn.execute(
        "UPDATE agent_recipes
         SET last_run_at = ?1,
             next_run_at = ?2,
             updated_at = ?1
         WHERE id = ?3",
        params![now, next_run_at, recipe_id],
    )?;
    Ok(())
}

pub fn list_runs(
    conn: &mut Connection,
    recipe_id: &str,
    limit: u32,
) -> CoreResult<Vec<AgentRecipeRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, recipe_id, bundle_id, triggered_at, status, error, created_at
         FROM agent_recipe_runs
         WHERE recipe_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![recipe_id, limit as i64], map_run_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::copilot_actions;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("recipes.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_update_and_list_round_trip() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let recipe = insert(
            &mut conn,
            "Monthly Budget Draft",
            "Draft next month budget",
            "monthly_budget_draft",
            "Review my spending and draft a budget.",
            "monthly",
            None,
            Some(1),
        )
        .unwrap();
        assert_eq!(recipe.status, "active");
        assert!(recipe.next_run_at.is_some());

        let updated = update(
            &mut conn,
            &recipe.id,
            "Budget Refresh",
            "Refresh monthly budget draft",
            "Refresh my budget draft.",
            "weekly",
            Some(2),
            None,
        )
        .unwrap();
        assert_eq!(updated.title, "Budget Refresh");
        assert_eq!(updated.cadence, "weekly");
        assert_eq!(updated.day_of_week, Some(2));
        assert_eq!(updated.day_of_month, None);

        let active_only = list(&mut conn, false).unwrap();
        assert_eq!(active_only.len(), 1);

        set_status(&mut conn, &recipe.id, "paused").unwrap();
        assert!(list(&mut conn, false).unwrap().is_empty());
        assert_eq!(list(&mut conn, true).unwrap()[0].status, "paused");
    }

    #[test]
    fn due_and_run_lifecycle_updates_recipe_state() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let recipe = insert(
            &mut conn,
            "Weekly Cleanup",
            "Clean up recent uncategorized transactions",
            "weekly_cleanup",
            "Suggest categories for recent flagged transactions.",
            "weekly",
            Some(0),
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_recipes SET next_run_at = '2000-01-01T09:00:00+00:00' WHERE id = ?1",
            params![&recipe.id],
        )
        .unwrap();

        let due = list_due(&mut conn).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, recipe.id);

        let run = start_run(&mut conn, &recipe.id).unwrap();
        let bundle = copilot_actions::insert_bundle(
            &mut conn,
            None,
            "Bundle",
            "Summary",
            "Rationale",
            0.9,
            Some("mock"),
            Some("test"),
        )
        .unwrap();
        complete_run(&mut conn, &run.id, &bundle.id).unwrap();

        let after_success = get(&mut conn, &recipe.id).unwrap().unwrap();
        assert_eq!(after_success.run_count, 1);
        assert!(after_success.last_run_at.is_some());
        assert!(after_success.next_run_at.is_some());

        let fail = start_run(&mut conn, &recipe.id).unwrap();
        fail_run(&mut conn, &fail.id, "llm exploded").unwrap();

        let runs = list_runs(&mut conn, &recipe.id, 10).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].status, "failed");
        assert_eq!(runs[1].status, "completed");
    }
}
