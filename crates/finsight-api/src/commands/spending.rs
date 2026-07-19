use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::repos::run;
use finsight_core::spending::classify::{self, PeriodAssessment};
use finsight_core::spending::plan::{self, SpendingPlan};
use finsight_core::spending::{annotate, baseline};
use serde::Serialize;
use specta::Type;

/// Everything the Path Back screen needs, in one read.
#[derive(Debug, Clone, Serialize, Type)]
pub struct PathBackView {
    pub period: String,
    pub assessment: PeriodAssessment,
    pub plan: SpendingPlan,
}

pub async fn get_spending_path_back(
    state: &ApiState,
    period: Option<String>,
    target_monthly_cents: Option<i64>,
) -> AppResult<Option<PathBackView>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let period = match period {
            Some(p) if p.len() >= 7 => p,
            _ => match baseline::latest_activity_month(conn)? {
                Some(ym) => ym,
                None => return Ok(None),
            },
        };
        let assessment = classify::classify_spending_period(conn, &period)?;
        let plan = plan::plan_spending_reduction(conn, &period, target_monthly_cents)?;
        Ok(Some(PathBackView { period, assessment, plan }))
    })
    .await
    .map_err(AppError::from)
}

pub async fn set_spending_annotation(
    state: &ApiState,
    merchant_key: String,
    verdict: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        if verdict == "reset" {
            annotate::clear_annotation(conn, &merchant_key)?;
        } else if annotate::VERDICTS.contains(&verdict.as_str()) {
            annotate::set_annotation(conn, &merchant_key, &verdict, None)?;
        }
        Ok(())
    })
    .await
    .map_err(AppError::from)
}
