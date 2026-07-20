//! Restoration envelopes — see `finsight_core::repos::restoration` for why this
//! is a notional tab rather than a claim about where dollars are sitting.

use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::repos::restoration::{
    self, NewRestorationEnvelope, RestorationEnvelope, RestorationLeg, RestorationStatus,
};
use finsight_core::repos::run;
use serde::Deserialize;
use specta::Type;

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestorationEnvelopeInput {
    pub label: String,
    pub source_account_id: Option<String>,
    pub destination_account_id: Option<String>,
    pub original_cents: i64,
    /// ISO date the money left the pot.
    pub opened_on: String,
    /// Optional `%name%` pattern for the person expected to pay some of it
    /// back. One person deliberately — see the repo docs.
    pub counterparty_pattern: Option<String>,
    pub note: Option<String>,
}

pub async fn list_restoration_envelopes(state: &ApiState) -> AppResult<Vec<RestorationEnvelope>> {
    let db = (*state.db).clone();
    run(&db, move |conn| restoration::list_open(conn))
        .await
        .map_err(AppError::from)
}

/// The three reliable numbers plus the honest ceiling, for one envelope.
pub async fn get_restoration_status(
    state: &ApiState,
    id: String,
) -> AppResult<Option<RestorationStatus>> {
    let db = (*state.db).clone();
    run(&db, move |conn| restoration::status(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn create_restoration_envelope(
    state: &ApiState,
    input: RestorationEnvelopeInput,
) -> AppResult<RestorationEnvelope> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        restoration::create(
            conn,
            NewRestorationEnvelope {
                label: input.label,
                source_account_id: input.source_account_id,
                destination_account_id: input.destination_account_id,
                original_cents: input.original_cents,
                opened_on: input.opened_on,
                counterparty_pattern: input.counterparty_pattern,
                note: input.note,
            },
        )
    })
    .await
    .map_err(AppError::from)
}

/// Reconcile and finish. The design nags toward this rather than letting
/// envelopes accumulate.
pub async fn close_restoration_envelope(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| restoration::close(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn delete_restoration_envelope(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| restoration::delete(conn, &id))
        .await
        .map_err(AppError::from)
}

/// Attribute money that has gone back into the pot.
pub async fn add_restoration_leg(
    state: &ApiState,
    envelope_id: String,
    amount_cents: i64,
    noted_on: String,
    transaction_id: Option<String>,
) -> AppResult<RestorationLeg> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        restoration::add_leg(
            conn,
            &envelope_id,
            amount_cents,
            &noted_on,
            transaction_id.as_deref(),
        )
    })
    .await
    .map_err(AppError::from)
}

pub async fn remove_restoration_leg(state: &ApiState, leg_id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| restoration::remove_leg(conn, &leg_id))
        .await
        .map_err(AppError::from)
}
