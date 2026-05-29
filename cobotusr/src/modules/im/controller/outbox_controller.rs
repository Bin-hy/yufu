use axum::{
    extract::{Json, State},
    response::IntoResponse,
};

use crate::{
    AppState,
    modules::im::{
        model::protocol::{ApiResponse, DispatchOutboxRequest, DispatchOutboxResponse},
        service,
    },
};

pub async fn dispatch_outbox(
    State(state): State<AppState>,
    Json(req): Json<DispatchOutboxRequest>,
) -> impl IntoResponse {
    let limit = super::normalize_limit(req.limit, 100, 500);
    let sent_count =
        match service::dispatch_due(&state.database.connection, state.ws_handle.clone(), limit)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return super::internal_error::<DispatchOutboxResponse>(e);
            }
        };

    ApiResponse::ok(DispatchOutboxResponse {
        limit,
        sent_count: sent_count as u64,
    })
}
