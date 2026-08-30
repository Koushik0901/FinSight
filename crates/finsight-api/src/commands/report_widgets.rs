use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{
    CreateReportWidgetRequest, DeleteReportWidgetRequest, ReorderReportWidgetsRequest,
    ReportWidget, UpdateReportWidgetRequest,
};
use finsight_core::repos::{report_widgets, run};



// ── Handlers ───────────────────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/rpc/list_report_widgets", responses((status = 200, body = Vec<ReportWidget>)))]
pub async fn list_report_widgets(state: &ApiState) -> AppResult<Vec<ReportWidget>> {
    let db = (*state.db).clone();
    run(&db, move |conn| report_widgets::list_widgets(&*conn))
        .await
        .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/create_report_widget", request_body(content = CreateReportWidgetRequest), responses((status = 200, body = ReportWidget)))]
pub async fn create_report_widget(
    state: &ApiState,
    title: String,
    chart_type: String,
    split_by: String,
    period: String,
    filters_json: Option<String>,
    position: Option<i64>,
) -> AppResult<ReportWidget> {
    let db = (*state.db).clone();
    run(
        &db,
        move |conn| {
            report_widgets::create_widget(
                conn,
                &title,
                &chart_type,
                &split_by,
                &period,
                filters_json.as_deref(),
                position,
            )
        },
    )
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/update_report_widget", request_body(content = UpdateReportWidgetRequest), responses((status = 200, body = Option<ReportWidget>)))]
pub async fn update_report_widget(
    state: &ApiState,
    id: String,
    title: Option<String>,
    chart_type: Option<String>,
    split_by: Option<String>,
    period: Option<String>,
    filters_json: Option<String>,
) -> AppResult<Option<ReportWidget>> {
    let db = (*state.db).clone();
    run(
        &db,
        move |conn| {
            report_widgets::update_widget(
                conn,
                &id,
                title.as_deref(),
                chart_type.as_deref(),
                split_by.as_deref(),
                period.as_deref(),
                filters_json.as_deref(),
            )
        },
    )
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/delete_report_widget", request_body(content = DeleteReportWidgetRequest), responses((status = 200, content_type = "application/json", body = bool)))]
pub async fn delete_report_widget(state: &ApiState, id: String) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| report_widgets::delete_widget(conn, &id))
        .await
        .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/reorder_report_widgets", request_body(content = ReorderReportWidgetsRequest), responses((status = 200, body = Vec<ReportWidget>)))]
pub async fn reorder_report_widgets(
    state: &ApiState,
    ordered_ids: Vec<String>,
) -> AppResult<Vec<ReportWidget>> {
    let db = (*state.db).clone();
    run(
        &db,
        move |conn| report_widgets::reorder_widgets(conn, &ordered_ids),
    )
    .await
    .map_err(AppError::from)
}
