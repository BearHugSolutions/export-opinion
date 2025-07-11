use anyhow::{Context, Result};
use log::info;
use tokio_postgres::Client;
use serde::{Deserialize, Serialize};

use crate::db_connect::PgPool;
use crate::team_utils::{TeamInfo, create_dataset_filter_clause};

const TEAM_SCHEMA: &str = "wa211_to_wric";

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewStats {
    pub pending_review: i64,
    pub confirmed_match: i64,
    pub confirmed_non_match: i64,
    pub total: i64,
    pub reviewed_count: i64,
    pub review_percentage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserDashboard {
    pub username: String,
    pub user_prefix: String,
    pub entity_stats: ReviewStats,
    pub service_stats: ReviewStats,
}

impl ReviewStats {
    fn new(pending: i64, confirmed_match: i64, confirmed_non_match: i64) -> Self {
        let total = pending + confirmed_match + confirmed_non_match;
        let reviewed_count = confirmed_match + confirmed_non_match;
        let review_percentage = if total > 0 {
            (reviewed_count as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        ReviewStats {
            pending_review: pending,
            confirmed_match,
            confirmed_non_match,
            total,
            reviewed_count,
            review_percentage,
        }
    }

    fn is_complete(&self) -> bool {
        self.pending_review == 0 && self.total > 0
    }
}

/// Fetches dashboard data for all users - used for Excel export progress overview
/// Now filters by team's whitelisted datasets
pub async fn get_dashboard_data(pool: &PgPool, team_info: &TeamInfo) -> Result<Vec<UserDashboard>> {
    info!("Fetching dashboard data for all users with dataset filtering...");

    // Define users - in a real app, this might come from a config or database
    let users = vec![
        ("Hannah", "hannah"),
        ("DrewW", "dreww"),
    ];

    let mut user_dashboards = Vec::new();

    for (username, user_prefix) in users {
        let client = pool.get().await.context("Failed to get DB client for dashboard")?;
        
        // Get entity review stats with dataset filtering
        let entity_stats = get_review_stats(&client, user_prefix, "entity", &team_info.whitelisted_datasets).await
            .with_context(|| format!("Failed to get entity stats for user {}", username))?;
        
        // Get service review stats with dataset filtering
        let service_stats = get_review_stats(&client, user_prefix, "service", &team_info.whitelisted_datasets).await
            .with_context(|| format!("Failed to get service stats for user {}", username))?;

        user_dashboards.push(UserDashboard {
            username: username.to_string(),
            user_prefix: user_prefix.to_string(),
            entity_stats,
            service_stats,
        });

        info!("Collected stats for user: {} (filtered by whitelisted datasets)", username);
    }

    Ok(user_dashboards)
}

/// Fetches review statistics for a specific user and record type (entity or service)
/// Now includes filtering by whitelisted datasets
async fn get_review_stats(
    client: &Client,
    user_prefix: &str,
    record_type: &str, // "entity" or "service"
    whitelisted_datasets: &[String],
) -> Result<ReviewStats> {
    let table_name = format!("{}_{}_edge_visualization", user_prefix, record_type);
    
    // Determine which ID columns and source table to use for filtering
    let (id_column_1, id_column_2, source_table, source_column) = match record_type {
        "entity" => ("entity_id_1", "entity_id_2", "entity", "source_system"),
        "service" => ("service_id_1", "service_id_2", "service", "source_system"),
        _ => return Err(anyhow::anyhow!("Invalid record type: {}", record_type)),
    };

    // Create dataset filter clause
    let (dataset_filter, filter_params) = create_dataset_filter_clause(
        "src", source_column, whitelisted_datasets, 1
    );

    let query = format!(
        r#"
        SELECT 
            ev.confirmed_status,
            COUNT(*) as count
        FROM "{}"."{}" ev
        INNER JOIN public.{} src ON (src.id = ev.{} OR src.id = ev.{})
        WHERE ev.confirmed_status IS NOT NULL 
        AND {}
        GROUP BY ev.confirmed_status
        "#,
        TEAM_SCHEMA, table_name, source_table, id_column_1, id_column_2, dataset_filter
    );

    // Convert filter_params to Vec<&(dyn ToSql + Sync)>
    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = filter_params
        .iter()
        .map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    let rows = client.query(&query, &params).await
        .context(format!("Failed to query {} edge visualization stats with dataset filtering", record_type))?;

    let mut pending_review = 0i64;
    let mut confirmed_match = 0i64;
    let mut confirmed_non_match = 0i64;

    for row in rows {
        let status: String = row.get("confirmed_status");
        let count: i64 = row.get("count");
        
        match status.as_str() {
            "PENDING_REVIEW" => pending_review = count,
            "CONFIRMED_MATCH" => confirmed_match = count,
            "CONFIRMED_NON_MATCH" => confirmed_non_match = count,
            _ => {}, // Ignore other statuses
        }
    }

    Ok(ReviewStats::new(pending_review, confirmed_match, confirmed_non_match))
}