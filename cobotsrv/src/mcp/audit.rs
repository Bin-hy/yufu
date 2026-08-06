//! MCP `tools/call` audit logging: write records and query them back.

use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use crate::entity::mcp_call_log::{self, McpCallLogVO};
use crate::entity::{PageSelectQuery, PageSelectQueryResult};

/// Fields captured for one audited call.
pub struct CallRecord {
    pub subject: String,
    pub method: String,
    pub tool_name: Option<String>,
    /// Error message if failed, `None` if success.
    pub error_msg: Option<String>,
    pub duration_ms: i64,
    pub request_id: Option<String>,
}

/// Insert one audit record. Errors are logged, not propagated — auditing must
/// never break the request path (callers typically run this in a background
/// writer task fed by a bounded channel).
pub async fn record(db: &DatabaseConnection, rec: CallRecord) {
    let row = mcp_call_log::ActiveModel {
        id: NotSet,
        created_at: Set(chrono::Utc::now().naive_utc()),
        subject: Set(rec.subject),
        method: Set(rec.method),
        tool_name: Set(rec.tool_name),
        error_msg: Set(rec.error_msg),
        duration_ms: Set(rec.duration_ms),
        request_id: Set(rec.request_id),
    };
    if let Err(e) = mcp_call_log::Entity::insert(row).exec(db).await {
        tracing::warn!(error = %e, "failed to write MCP audit log");
    }
}

/// Delete audit records older than `days`. Returns the number of rows deleted.
pub async fn purge_older_than(db: &DatabaseConnection, days: i64) -> Result<u64, sea_orm::DbErr> {
    let cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::days(days);
    let res = mcp_call_log::Entity::delete_many()
        .filter(mcp_call_log::Column::CreatedAt.lt(cutoff))
        .exec(db)
        .await?;
    Ok(res.rows_affected)
}

/// Filters for the audit query endpoint.
#[derive(Default)]
pub struct LogFilter {
    pub subject: Option<String>,
    pub tool: Option<String>,
    /// Some(true) → only errors, Some(false) → only successes, None → all.
    pub is_error: Option<bool>,
}

/// Paginated, newest-first query of audit records.
pub async fn query(
    db: &DatabaseConnection,
    page: &PageSelectQuery,
    filter: &LogFilter,
) -> Result<PageSelectQueryResult<McpCallLogVO>, sea_orm::DbErr> {
    let mut q = mcp_call_log::Entity::find();
    if let Some(s) = &filter.subject {
        q = q.filter(mcp_call_log::Column::Subject.eq(s.clone()));
    }
    if let Some(t) = &filter.tool {
        q = q.filter(mcp_call_log::Column::ToolName.eq(t.clone()));
    }
    // error_msg IS NOT NULL → error; IS NULL → success
    if let Some(err) = filter.is_error {
        if err {
            q = q.filter(mcp_call_log::Column::ErrorMsg.is_not_null());
        } else {
            q = q.filter(mcp_call_log::Column::ErrorMsg.is_null());
        }
    }

    // Newest first; `id` breaks `created_at` ties so pagination is stable.
    let paginator = q
        .order_by_desc(mcp_call_log::Column::CreatedAt)
        .order_by_desc(mcp_call_log::Column::Id)
        .paginate(db, page.page_size.clamp(1, 200));
    let total_pages = paginator.num_pages().await?;
    let rows = paginator
        .fetch_page(page.page_number.saturating_sub(1))
        .await?;

    Ok(PageSelectQueryResult {
        page_size: page.page_size,
        page_number: page.page_number,
        total_pages,
        data_list: rows.into_iter().map(McpCallLogVO::from).collect(),
    })
}

/// (total, success) call counts since UTC midnight today.
pub async fn today_stats(db: &DatabaseConnection) -> Result<(u64, u64), sea_orm::DbErr> {
    let start = chrono::Utc::now()
        .date_naive()
        .and_time(chrono::NaiveTime::MIN);
    let base = mcp_call_log::Entity::find().filter(mcp_call_log::Column::CreatedAt.gte(start));
    let total = base.clone().count(db).await?;
    let success = base
        .filter(mcp_call_log::Column::ErrorMsg.is_null())
        .count(db)
        .await?;
    Ok((total, success))
}
