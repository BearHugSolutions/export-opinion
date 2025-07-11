// team_utils.rs
use anyhow::{Context, Result};
use log::info;
use tokio_postgres::Client;
use serde::{Deserialize, Serialize};

use crate::db_connect::PgPool;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub whitelisted_datasets: Vec<String>,
    pub is_active: bool,
}

/// Fetches team information and whitelisted datasets for the given users
pub async fn get_team_info_for_users(pool: &PgPool, user_prefixes: &[&str]) -> Result<TeamInfo> {
    info!("Fetching team information for users: {:?}", user_prefixes);

    let client = pool.get().await.context("Failed to get DB client for team info")?;

    // First, let's try to find the team by looking up one of the users
    // We'll use the first user prefix to find the team
    let first_user_prefix = user_prefixes.first()
        .ok_or_else(|| anyhow::anyhow!("No user prefixes provided"))?;

    let query = r#"
        SELECT t.id, t.name, t.display_name, t.whitelisted_datasets, t.is_active
        FROM public.teams t
        JOIN public.users u ON u.team_id = t.id
        WHERE u.user_opinion_prefix = $1
        LIMIT 1
    "#;

    let row = client.query_opt(query, &[first_user_prefix]).await
        .context("Failed to query team information")?
        .ok_or_else(|| anyhow::anyhow!("No team found for user prefix: {}", first_user_prefix))?;

    let team_info = TeamInfo {
        id: row.get("id"),
        name: row.get("name"),
        display_name: row.get("display_name"),
        whitelisted_datasets: row.get("whitelisted_datasets"),
        is_active: row.get("is_active"),
    };

    info!(
        "Found team '{}' with whitelisted datasets: {:?}",
        team_info.name, team_info.whitelisted_datasets
    );

    // Verify all users belong to the same team
    for user_prefix in user_prefixes.iter().skip(1) {
        let user_team_query = r#"
            SELECT t.id
            FROM public.teams t
            JOIN public.users u ON u.team_id = t.id
            WHERE u.user_opinion_prefix = $1
        "#;

        let user_team_row = client.query_opt(user_team_query, &[user_prefix]).await
            .context(format!("Failed to query team for user: {}", user_prefix))?;

        match user_team_row {
            Some(row) => {
                let user_team_id: String = row.get("id");
                if user_team_id != team_info.id {
                    return Err(anyhow::anyhow!(
                        "User '{}' belongs to different team '{}', expected '{}'",
                        user_prefix, user_team_id, team_info.id
                    ));
                }
            }
            None => {
                return Err(anyhow::anyhow!("No team found for user prefix: {}", user_prefix));
            }
        }
    }

    Ok(team_info)
}

/// Helper function to create WHERE clause for filtering by whitelisted datasets
pub fn create_dataset_filter_clause(
    table_alias: &str,
    column_name: &str,
    whitelisted_datasets: &[String],
    param_start_index: usize,
) -> (String, Vec<String>) {
    if whitelisted_datasets.is_empty() {
        return ("1=1".to_string(), vec![]);
    }

    let placeholders: Vec<String> = (param_start_index..param_start_index + whitelisted_datasets.len())
        .map(|i| format!("${}", i))
        .collect();

    let where_clause = format!(
        "{}.{} = ANY(ARRAY[{}])",
        table_alias,
        column_name,
        placeholders.join(", ")
    );

    (where_clause, whitelisted_datasets.to_vec())
}