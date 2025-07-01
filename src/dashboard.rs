use anyhow::{Context, Result};
use log::info;
use tokio_postgres::Client;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

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

/// Fetches review statistics for all users and generates an HTML dashboard
pub async fn generate_dashboard(pool: &PgPool, output_path: &Path) -> Result<()> {
    info!("Generating review dashboard...");

    let user_dashboards = get_dashboard_data(pool).await?;

    // Generate HTML dashboard
    let html_content = generate_html_dashboard(&user_dashboards)?;
    
    // Write to file
    fs::write(output_path, html_content).await
        .context("Failed to write dashboard HTML file")?;

    info!("Dashboard generated successfully at: {:?}", output_path);
    Ok(())
}

/// Fetches dashboard data for all users - can be used for both HTML dashboard and Excel export
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

/// Generates the HTML content for the dashboard
fn generate_html_dashboard(user_dashboards: &[UserDashboard]) -> Result<String> {
    let mut html = String::new();
    
    // HTML header
    html.push_str(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Edge Review Dashboard</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #f5f5f5;
            color: #333;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 12px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            overflow: hidden;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
            text-align: center;
        }
        .header h1 {
            margin: 0;
            font-size: 2rem;
            font-weight: 300;
        }
        .header p {
            margin: 10px 0 0 0;
            opacity: 0.9;
        }
        .user-section {
            border-bottom: 1px solid #e0e0e0;
            padding: 30px;
        }
        .user-section:last-child {
            border-bottom: none;
        }
        .user-header {
            display: flex;
            align-items: center;
            margin-bottom: 25px;
        }
        .user-name {
            font-size: 1.5rem;
            font-weight: 600;
            color: #2c3e50;
            margin-right: 15px;
        }
        .user-prefix {
            background: #ecf0f1;
            color: #7f8c8d;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 0.8rem;
            font-family: monospace;
        }
        .record-types {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 30px;
        }
        .record-type {
            background: #fafafa;
            border-radius: 8px;
            padding: 20px;
            border-left: 4px solid #3498db;
        }
        .record-type.service {
            border-left-color: #e74c3c;
        }
        .record-type-title {
            font-size: 1.1rem;
            font-weight: 600;
            margin-bottom: 15px;
            text-transform: capitalize;
        }
        .stat-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 15px;
            margin-bottom: 20px;
        }
        .stat-item {
            background: white;
            padding: 15px;
            border-radius: 6px;
            text-align: center;
            box-shadow: 0 2px 4px rgba(0,0,0,0.05);
        }
        .stat-value {
            font-size: 1.8rem;
            font-weight: 700;
            margin-bottom: 5px;
        }
        .stat-label {
            font-size: 0.8rem;
            color: #7f8c8d;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }
        .progress-container {
            background: #ecf0f1;
            border-radius: 10px;
            overflow: hidden;
            margin-bottom: 10px;
        }
        .progress-bar {
            height: 20px;
            background: linear-gradient(90deg, #27ae60, #2ecc71);
            transition: width 0.3s ease;
            display: flex;
            align-items: center;
            justify-content: center;
            color: white;
            font-size: 0.8rem;
            font-weight: 600;
        }
        .progress-text {
            text-align: center;
            font-size: 0.9rem;
            color: #555;
        }
        .status-complete {
            color: #27ae60;
            font-weight: 600;
        }
        .status-pending {
            color: #e67e22;
            font-weight: 600;
        }
        .summary {
            background: #f8f9fa;
            padding: 20px;
            margin: 20px 30px;
            border-radius: 8px;
            border: 1px solid #dee2e6;
        }
        .summary h3 {
            margin: 0 0 15px 0;
            color: #495057;
        }
        .summary-stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 15px;
        }
        @media (max-width: 768px) {
            .record-types {
                grid-template-columns: 1fr;
            }
            .stat-grid {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Edge Review Dashboard</h1>
            <p>Track progress on human review of entity and service edge visualizations</p>
        </div>
"#);

    // Calculate summary statistics
    let mut total_entity_pending = 0i64;
    let mut total_entity_reviewed = 0i64;
    let mut total_service_pending = 0i64;
    let mut total_service_reviewed = 0i64;

    for user in user_dashboards {
        total_entity_pending += user.entity_stats.pending_review;
        total_entity_reviewed += user.entity_stats.reviewed_count;
        total_service_pending += user.service_stats.pending_review;
        total_service_reviewed += user.service_stats.reviewed_count;
    }

    let total_pending = total_entity_pending + total_service_pending;
    let total_reviewed = total_entity_reviewed + total_service_reviewed;
    let total_all = total_pending + total_reviewed;
    let overall_percentage = if total_all > 0 {
        (total_reviewed as f64 / total_all as f64) * 100.0
    } else {
        0.0
    };

    // Add summary section
    html.push_str(&format!(
        r#"
        <div class="summary">
            <h3>Overall Progress</h3>
            <div class="summary-stats">
                <div class="stat-item">
                    <div class="stat-value">{}</div>
                    <div class="stat-label">Total Pending</div>
                </div>
                <div class="stat-item">
                    <div class="stat-value">{}</div>
                    <div class="stat-label">Total Reviewed</div>
                </div>
                <div class="stat-item">
                    <div class="stat-value">{:.1}%</div>
                    <div class="stat-label">Overall Complete</div>
                </div>
                <div class="stat-item">
                    <div class="stat-value">{}</div>
                    <div class="stat-label">Total Records</div>
                </div>
            </div>
        </div>
        "#,
        total_pending, total_reviewed, overall_percentage, total_all
    ));

    // Add user sections
    for user in user_dashboards {
        html.push_str(&format!(
            r#"
        <div class="user-section">
            <div class="user-header">
                <div class="user-name">{}</div>
                <div class="user-prefix">{}</div>
            </div>
            <div class="record-types">
            "#,
            user.username, user.user_prefix
        ));

        // Entity section
        html.push_str(&generate_record_type_html("entity", &user.entity_stats));

        // Service section  
        html.push_str(&generate_record_type_html("service", &user.service_stats));

        html.push_str("            </div>\n        </div>");
    }

    // HTML footer
    html.push_str(&format!(
        r#"
    </div>
    <script>
        // Auto-refresh every 5 minutes
        setTimeout(() => {{
            window.location.reload();
        }}, 300000);
        
        // Add timestamp
        document.addEventListener('DOMContentLoaded', function() {{
            const now = new Date();
            const timestamp = document.createElement('div');
            timestamp.style.textAlign = 'center';
            timestamp.style.padding = '20px';
            timestamp.style.color = '#7f8c8d';
            timestamp.style.fontSize = '0.9rem';
            timestamp.textContent = 'Last updated: ' + now.toLocaleString();
            document.body.appendChild(timestamp);
        }});
    </script>
</body>
</html>
        "#
    ));

    Ok(html)
}

/// Generates HTML for a specific record type (entity or service)
fn generate_record_type_html(record_type: &str, stats: &ReviewStats) -> String {
    let type_class = if record_type == "service" { "service" } else { "" };
    let status_class = if stats.is_complete() { "status-complete" } else { "status-pending" };
    let status_text = if stats.is_complete() { 
        "✓ Review Complete" 
    } else { 
        &format!("{} reviews remaining", stats.pending_review)
    };

    format!(
        r#"
                <div class="record-type {}">
                    <div class="record-type-title">{} Edges</div>
                    <div class="stat-grid">
                        <div class="stat-item">
                            <div class="stat-value">{}</div>
                            <div class="stat-label">Pending Review</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">{}</div>
                            <div class="stat-label">Confirmed Match</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">{}</div>
                            <div class="stat-label">Confirmed Non-Match</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">{}</div>
                            <div class="stat-label">Total Edges</div>
                        </div>
                    </div>
                    <div class="progress-container">
                        <div class="progress-bar" style="width: {:.1}%;">
                            {:.1}%
                        </div>
                    </div>
                    <div class="progress-text {}">
                        {}
                    </div>
                </div>
        "#,
        type_class,
        record_type.to_uppercase(),
        stats.pending_review,
        stats.confirmed_match,
        stats.confirmed_non_match,
        stats.total,
        stats.review_percentage,
        stats.review_percentage,
        status_class,
        status_text
    )
}