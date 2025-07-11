use anyhow::{Context, Result};
use log::info;
use tokio_postgres::Client;
use serde::{Deserialize, Serialize};

use crate::db_connect::PgPool;

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
pub async fn get_dashboard_data(pool: &PgPool) -> Result<Vec<UserDashboard>> {
    info!("Fetching dashboard data for all users...");

    // Define users - in a real app, this might come from a config or database
    let users = vec![
        ("Hannah", "hannah"),
        ("DrewW", "dreww"),
    ];

    let mut user_dashboards = Vec::new();

    for (username, user_prefix) in users {
        let client = pool.get().await.context("Failed to get DB client for dashboard")?;
        
        // Get entity review stats
        let entity_stats = get_review_stats(&client, user_prefix, "entity").await
            .with_context(|| format!("Failed to get entity stats for user {}", username))?;
        
        // Get service review stats
        let service_stats = get_review_stats(&client, user_prefix, "service").await
            .with_context(|| format!("Failed to get service stats for user {}", username))?;

        user_dashboards.push(UserDashboard {
            username: username.to_string(),
            user_prefix: user_prefix.to_string(),
            entity_stats,
            service_stats,
        });

        info!("Collected stats for user: {}", username);
    }

    Ok(user_dashboards)
}

/// Fetches review statistics for a specific user and record type (entity or service)
async fn get_review_stats(
    client: &Client,
    user_prefix: &str,
    record_type: &str, // "entity" or "service"
) -> Result<ReviewStats> {
    let table_name = format!("{}_{}_edge_visualization", user_prefix, record_type);
    
    let query = format!(
        r#"
        SELECT 
            confirmed_status,
            COUNT(*) as count
        FROM "{}"."{}"
        WHERE confirmed_status IS NOT NULL
        GROUP BY confirmed_status
        "#,
        TEAM_SCHEMA, table_name
    );

    let rows = client.query(&query, &[]).await
        .context(format!("Failed to query {} edge visualization stats", record_type))?;

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