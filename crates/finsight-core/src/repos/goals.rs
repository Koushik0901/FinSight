use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

/// How much a goal matters relative to the others, as the USER sees it.
///
/// Distinct from `sort_order`, which is where they dragged the card. Dragging a
/// list around must not silently change financial advice, so importance is its
/// own axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GoalPriority {
    /// Fund before anything else — the emergency fund, a tax bill.
    Critical,
    High,
    /// The default. Every goal that existed before priorities did.
    Normal,
    /// Aspirational. Fund from what is genuinely spare.
    Someday,
}

impl GoalPriority {
    /// Parse a stored value, falling back to `Normal` for anything
    /// unrecognised. A goal whose priority we cannot read must still be
    /// planned around — dropping it, or refusing to rank it, would lose the
    /// user's money from their own plan over a typo.
    pub fn from_db(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" => Self::High,
            "someday" => Self::Someday,
            _ => Self::Normal,
        }
    }

    pub fn as_db(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Normal => "normal",
            Self::Someday => "someday",
        }
    }

    /// Sort key: lower comes first. Derived from the declaration order above,
    /// so adding a level in the right place is all it takes.
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// What the goal's `target_date` actually commits the user to.
///
/// The same date can be an immovable obligation or a hope, and allocation
/// should not treat a wedding like "pay off the car by summer".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineStrictness {
    /// Immovable: a wedding, a visa fee, a tax instalment. Missing it costs
    /// something real beyond disappointment.
    Hard,
    /// The default — a date the user is aiming at.
    Target,
    /// No meaningful deadline.
    None,
}

impl DeadlineStrictness {
    pub fn from_db(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "hard" => Self::Hard,
            "none" => Self::None,
            _ => Self::Target,
        }
    }

    pub fn as_db(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Target => "target",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Goal {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub current_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub account_id: Option<String>,
    pub priority: GoalPriority,
    pub deadline_strictness: DeadlineStrictness,
}

impl Goal {
    /// What the deadline actually means, reconciling the stored strictness with
    /// whether a date exists at all.
    ///
    /// A goal with no `target_date` is open-ended whatever the column says —
    /// the two can disagree (set a hard date, then clear the date) and the
    /// stored value alone would have planning treat a nonexistent deadline as
    /// immovable.
    pub fn effective_strictness(&self) -> DeadlineStrictness {
        match self.target_date.as_deref().map(str::trim) {
            None | Some("") => DeadlineStrictness::None,
            Some(_) => self.deadline_strictness,
        }
    }

    /// Ordering key for "what should I fund first": priority, then whether the
    /// deadline is immovable, then the soonest date.
    ///
    /// Every goal that predates this feature sorts identically to every other,
    /// which is what makes the new ordering collapse back to the old
    /// date-based one when nobody has expressed a preference.
    pub fn funding_order_key(&self) -> (u8, u8, Option<String>) {
        let hard_first = match self.effective_strictness() {
            DeadlineStrictness::Hard => 0,
            _ => 1,
        };
        (self.priority.rank(), hard_first, self.target_date.clone())
    }
}

#[derive(Debug, Clone)]
pub struct NewGoal {
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub account_id: Option<String>,
    /// `None` uses the schema default (`normal` / `target`), so a caller that
    /// does not care about priority does not have to think about it.
    pub priority: Option<GoalPriority>,
    pub deadline_strictness: Option<DeadlineStrictness>,
}

// `GoalPatch` used to live here. It was dead — declared, never constructed,
// never read anywhere in the workspace — while goals are actually edited
// through narrow intent-named commands (`update_goal_monthly`,
// `update_goal_balance`, and now `set_goal_priority`). Removed rather than
// grown, because a patch struct nothing reads is a standing invitation to add
// a field to it and wonder why nothing happens.

pub fn list(conn: &mut Connection) -> CoreResult<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, target_cents, current_cents, monthly_cents, \
                target_date, color, notes, purpose, sort_order, created_at, \
                account_id, priority, deadline_strictness \
         FROM goals WHERE archived_at IS NULL ORDER BY sort_order, created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Goal {
            id: r.get(0)?,
            name: r.get(1)?,
            goal_type: r.get(2)?,
            target_cents: r.get(3)?,
            current_cents: r.get(4)?,
            monthly_cents: r.get(5)?,
            target_date: r.get(6)?,
            color: r.get(7)?,
            notes: r.get(8)?,
            purpose: r.get(9)?,
            sort_order: r.get(10)?,
            created_at: r.get(11)?,
            account_id: r.get(12)?,
            priority: GoalPriority::from_db(&r.get::<_, String>(13)?),
            deadline_strictness: DeadlineStrictness::from_db(&r.get::<_, String>(14)?),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Goal> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, target_cents, current_cents, monthly_cents, \
                target_date, color, notes, purpose, sort_order, created_at, \
                account_id, priority, deadline_strictness \
         FROM goals WHERE id = ?1 AND archived_at IS NULL",
    )?;
    let mut rows = stmt.query_map(params![id], |r| {
        Ok(Goal {
            id: r.get(0)?,
            name: r.get(1)?,
            goal_type: r.get(2)?,
            target_cents: r.get(3)?,
            current_cents: r.get(4)?,
            monthly_cents: r.get(5)?,
            target_date: r.get(6)?,
            color: r.get(7)?,
            notes: r.get(8)?,
            purpose: r.get(9)?,
            sort_order: r.get(10)?,
            created_at: r.get(11)?,
            account_id: r.get(12)?,
            priority: GoalPriority::from_db(&r.get::<_, String>(13)?),
            deadline_strictness: DeadlineStrictness::from_db(&r.get::<_, String>(14)?),
        })
    })?;
    rows.next()
        .transpose()?
        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
}

pub fn insert(conn: &mut Connection, g: NewGoal) -> CoreResult<Goal> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO goals(id, name, type, target_cents, current_cents, monthly_cents, \
                           target_date, color, notes, purpose, sort_order, created_at, \
                           account_id, priority, deadline_strictness)
         VALUES(?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12, ?13)",
        params![
            id,
            g.name,
            g.goal_type,
            g.target_cents,
            g.monthly_cents,
            g.target_date,
            g.color,
            g.notes,
            g.purpose,
            now,
            g.account_id,
            g.priority.unwrap_or(GoalPriority::Normal).as_db(),
            g.deadline_strictness
                .unwrap_or(DeadlineStrictness::Target)
                .as_db()
        ],
    )?;
    Ok(Goal {
        id,
        name: g.name,
        goal_type: g.goal_type,
        target_cents: g.target_cents,
        current_cents: 0,
        monthly_cents: g.monthly_cents,
        target_date: g.target_date,
        color: g.color,
        notes: g.notes,
        purpose: g.purpose,
        sort_order: 0,
        created_at: now,
        account_id: g.account_id,
        priority: g.priority.unwrap_or(GoalPriority::Normal),
        deadline_strictness: g.deadline_strictness.unwrap_or(DeadlineStrictness::Target),
    })
}

/// Record how much a goal matters and what its date commits the user to.
///
/// Narrow and intent-named, matching `update_goal_monthly` — these two travel
/// together in the UI and neither is meaningful without the other in the
/// planner.
pub fn set_priority(
    conn: &mut Connection,
    id: &str,
    priority: GoalPriority,
    deadline_strictness: DeadlineStrictness,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET priority = ?1, deadline_strictness = ?2 WHERE id = ?3",
        params![priority.as_db(), deadline_strictness.as_db(), id],
    )?;
    Ok(())
}

/// Sync `current_cents` of every goal linked to the given account with the
/// account's current debt magnitude (the amount owed — i.e. the absolute
/// value of its latest negative balance; 0 if the account isn't in debt).
/// Replaces the old `sync_linked_liabilities`, called from `set_account_balance`
/// now that debt lives on Account instead of a separate `liabilities` table.
pub fn sync_linked_accounts(conn: &mut Connection, account_id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals
         SET current_cents = MAX(0, -COALESCE((
             SELECT balance_cents FROM account_balances b
             WHERE b.account_id = ?1
             ORDER BY b.as_of_date DESC,
                 CASE b.source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END
             LIMIT 1
         ), 0))
         WHERE account_id = ?1",
        params![account_id],
    )?;
    Ok(())
}

/// Directly set `current_cents`. Reserved for the account-linked sync path
/// ([`sync_linked_accounts`]), whose source of truth is the account balance.
/// For manual goals, use [`add_contribution`] instead — a direct set would
/// desync the contribution ledger.
pub fn set_current_cents(conn: &mut Connection, id: &str, current_cents: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET current_cents = ?1 WHERE id = ?2",
        params![current_cents, id],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct GoalContribution {
    pub id: String,
    pub goal_id: String,
    pub amount_cents: i64,
    pub note: Option<String>,
    pub source: String,
    pub created_at: String,
}

/// Append a contribution (positive) or withdrawal (negative) to a goal's ledger
/// and re-derive its `current_cents` as the sum of all its contributions. This
/// is the correct way to change a manual goal's balance: the ledger is the
/// source of truth, so parking twice appends two auditable rows instead of
/// double-counting, and nothing silently overwrites it. Account-linked goals
/// must NOT use this (their balance comes from the account) — callers guard it.
pub fn add_contribution(
    conn: &mut Connection,
    goal_id: &str,
    amount_cents: i64,
    note: Option<&str>,
    source: &str,
) -> CoreResult<GoalContribution> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO goal_contributions(id, goal_id, amount_cents, note, source, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, goal_id, amount_cents, note, source, now],
    )?;
    recompute_current_cents(conn, goal_id)?;
    Ok(GoalContribution {
        id,
        goal_id: goal_id.to_string(),
        amount_cents,
        note: note.map(str::to_string),
        source: source.to_string(),
        created_at: now,
    })
}

/// Re-derive `goals.current_cents` from the contribution ledger (the materialized
/// cache all read paths use).
pub fn recompute_current_cents(conn: &mut Connection, goal_id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET current_cents = (
             SELECT COALESCE(SUM(amount_cents), 0) FROM goal_contributions WHERE goal_id = ?1
         ) WHERE id = ?1",
        params![goal_id],
    )?;
    Ok(())
}

pub fn list_contributions(
    conn: &mut Connection,
    goal_id: &str,
) -> CoreResult<Vec<GoalContribution>> {
    let mut stmt = conn.prepare(
        "SELECT id, goal_id, amount_cents, note, source, created_at
         FROM goal_contributions WHERE goal_id = ?1 ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map(params![goal_id], |r| {
        Ok(GoalContribution {
            id: r.get(0)?,
            goal_id: r.get(1)?,
            amount_cents: r.get(2)?,
            note: r.get(3)?,
            source: r.get(4)?,
            created_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn archive(conn: &mut Connection, id: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE goals SET archived_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn set_monthly_cents(conn: &mut Connection, id: &str, monthly_cents: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET monthly_cents = ?1 WHERE id = ?2",
        params![monthly_cents, id],
    )?;
    Ok(())
}

pub fn set_purpose(conn: &mut Connection, id: &str, purpose: Option<&str>) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET purpose = ?1 WHERE id = ?2",
        params![purpose, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("g.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn set_monthly_cents_updates_correctly() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let goal = insert(
            &mut conn,
            NewGoal {
                priority: None,
                deadline_strictness: None,
                name: "Italy trip".into(),
                goal_type: "save-by-date".into(),
                target_cents: 500_000,
                monthly_cents: 10_000,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                account_id: None,
            },
        )
        .unwrap();
        set_monthly_cents(&mut conn, &goal.id, 25_000).unwrap();
        let updated = list(&mut conn)
            .unwrap()
            .into_iter()
            .find(|g| g.id == goal.id)
            .unwrap();
        assert_eq!(updated.monthly_cents, 25_000);
    }

    #[test]
    fn contributions_derive_current_balance_and_are_auditable() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let goal = insert(
            &mut conn,
            NewGoal {
                priority: None,
                deadline_strictness: None,
                name: "Emergency".into(),
                goal_type: "build-balance".into(),
                target_cents: 1_000_000,
                monthly_cents: 0,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                account_id: None,
            },
        )
        .unwrap();
        assert_eq!(goal.current_cents, 0);

        // Two parks append two rows; the balance is their sum (no double-count).
        add_contribution(&mut conn, &goal.id, 50_000, Some("Parked surplus"), "sweep").unwrap();
        add_contribution(&mut conn, &goal.id, 30_000, None, "manual").unwrap();
        let after_deposits = get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(after_deposits.current_cents, 80_000);

        // A withdrawal is a negative row; the derived balance reflects it.
        add_contribution(&mut conn, &goal.id, -20_000, Some("Pulled out"), "manual").unwrap();
        let after_withdraw = get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(after_withdraw.current_cents, 60_000);

        let ledger = list_contributions(&mut conn, &goal.id).unwrap();
        assert_eq!(ledger.len(), 3, "every movement is an auditable row");
        assert_eq!(ledger.iter().map(|c| c.amount_cents).sum::<i64>(), 60_000);
    }

    fn insert_debt_account(conn: &mut Connection, name: &str) -> String {
        use crate::models::{AccountType, NewAccount};
        use crate::repos::accounts;
        accounts::insert(
            conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "Household".into(),
                bank: "Manual".into(),
                r#type: AccountType::Loan,
                name: name.into(),
                last4: None,
                currency: "USD".into(),
                color: "#F87171".into(),
                opening_balance_cents: -5_000_00,
                source: "manual".into(),
                liquidity_type: "restricted".into(),
                emergency_fund_eligible: false,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "debt".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                limit_cents: None,
                original_balance_cents: None,
                started_at: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn insert_goal_with_account_link_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let account_id = insert_debt_account(&mut conn, "Loan");
        let goal = insert(
            &mut conn,
            NewGoal {
                priority: None,
                deadline_strictness: None,
                name: "Payoff".into(),
                goal_type: "debt-payoff".into(),
                target_cents: 5_000_00,
                monthly_cents: 100_00,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                account_id: Some(account_id.clone()),
            },
        )
        .unwrap();
        assert_eq!(goal.account_id, Some(account_id.clone()));
        let fetched = get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(fetched.account_id, Some(account_id));
    }

    #[test]
    fn deleting_account_clears_goal_link() {
        use crate::repos::accounts;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let account_id = insert_debt_account(&mut conn, "Loan");
        let goal = insert(
            &mut conn,
            NewGoal {
                priority: None,
                deadline_strictness: None,
                name: "Payoff".into(),
                goal_type: "debt-payoff".into(),
                target_cents: 5_000_00,
                monthly_cents: 100_00,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                account_id: Some(account_id.clone()),
            },
        )
        .unwrap();
        accounts::archive(&mut conn, &account_id).unwrap();
        conn.execute("DELETE FROM accounts WHERE id = ?1", params![account_id])
            .unwrap();
        let fetched = get_by_id(&mut conn, &goal.id).unwrap();
        assert!(fetched.account_id.is_none());
    }

    #[test]
    fn sync_linked_accounts_reflects_the_amount_owed() {
        use crate::repos::accounts;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // insert_debt_account seeds an opening balance of -$5,000.00.
        let account_id = insert_debt_account(&mut conn, "Car loan");
        let goal = insert(
            &mut conn,
            NewGoal {
                priority: None,
                deadline_strictness: None,
                name: "Pay off car".into(),
                goal_type: "debt-payoff".into(),
                target_cents: 5_000_00,
                monthly_cents: 500_00,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                account_id: Some(account_id.clone()),
            },
        )
        .unwrap();
        assert_eq!(goal.current_cents, 0);

        sync_linked_accounts(&mut conn, &account_id).unwrap();
        let synced = get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(synced.current_cents, 5_000_00, "amount owed is the positive magnitude of the negative balance");

        // Paying the debt down to $0 must sync the goal to 0, not go negative.
        let today = chrono::Utc::now().date_naive().to_string();
        accounts::upsert_balance_snapshot(&mut conn, &account_id, &today, 0, None, Some("manual")).unwrap();
        sync_linked_accounts(&mut conn, &account_id).unwrap();
        let paid_off = get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(paid_off.current_cents, 0);
    }
}
