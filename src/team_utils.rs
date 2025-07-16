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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub user_opinion_prefix: Option<String>,
    pub team_id: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpinionInfo {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub owner_username: String,
    pub other_users: Vec<String>,
    pub disconnect_dependent_services: bool,
}

/// Fetches all available teams from the auth schema
pub async fn get_all_teams(pool: &PgPool) -> Result<Vec<TeamInfo>> {
    info!("Fetching all teams from auth schema...");
    
    let client = pool.get().await.context("Failed to get DB client for teams")?;
    
    let query = r#"
        SELECT id, name, display_name, whitelisted_datasets, is_active
        FROM auth.teams
        WHERE is_active = true
        ORDER BY display_name
    "#;
    
    let rows = client.query(query, &[]).await
        .context("Failed to query teams from auth schema")?;
    
    let mut teams = Vec::new();
    for row in rows {
        teams.push(TeamInfo {
            id: row.get("id"),
            name: row.get("name"),
            display_name: row.get("display_name"),
            whitelisted_datasets: row.get("whitelisted_datasets"),
            is_active: row.get("is_active"),
        });
    }
    
    info!("Found {} active teams", teams.len());
    Ok(teams)
}

/// Fetches all users for a specific team from the auth schema
pub async fn get_users_for_team(pool: &PgPool, team_id: &str) -> Result<Vec<UserInfo>> {
    info!("Fetching users for team: {}", team_id);
    
    let client = pool.get().await.context("Failed to get DB client for users")?;
    
    let query = r#"
        SELECT id, username, email, user_opinion_prefix, team_id, is_active
        FROM auth.users
        WHERE team_id = $1 AND is_active = true
        ORDER BY username
    "#;
    
    let rows = client.query(query, &[&team_id]).await
        .context("Failed to query users from auth schema")?;
    
    let mut users = Vec::new();
    for row in rows {
        users.push(UserInfo {
            id: row.get("id"),
            username: row.get("username"),
            email: row.get("email"),
            user_opinion_prefix: row.get("user_opinion_prefix"),
            team_id: row.get("team_id"),
            is_active: row.get("is_active"),
        });
    }
    
    info!("Found {} active users for team", users.len());
    Ok(users)
}

/// Fetches all opinions accessible to a specific user from the auth schema
/// This includes opinions owned by the user and opinions shared with the user
pub async fn get_opinions_for_user(pool: &PgPool, user_id: &str) -> Result<Vec<OpinionInfo>> {
    info!("Fetching opinions for user: {}", user_id);
    
    let client = pool.get().await.context("Failed to get DB client for opinions")?;
    
    let query = r#"
        SELECT 
            o.id,
            o.name,
            o.user_id,
            u.username as owner_username,
            o.other_users,
            o.disconnectdependentservices
        FROM auth.opinions o
        JOIN auth.users u ON o.user_id = u.id
        WHERE o.user_id = $1 
           OR o.other_users ? $1
        ORDER BY o.name
    "#;
    
    let rows = client.query(query, &[&user_id]).await
        .context("Failed to query opinions from auth schema")?;
    
    let mut opinions = Vec::new();
    for row in rows {
        let other_users_json: serde_json::Value = row.get("other_users");
        let other_users: Vec<String> = serde_json::from_value(other_users_json)
            .unwrap_or_else(|_| vec![]);
        
        opinions.push(OpinionInfo {
            id: row.get("id"),
            name: row.get("name"),
            user_id: row.get("user_id"),
            owner_username: row.get("owner_username"),
            other_users,
            disconnect_dependent_services: row.get("disconnectdependentservices"),
        });
    }
    
    info!("Found {} accessible opinions for user", opinions.len());
    Ok(opinions)
}

/// Fetches team information by team ID from the auth schema
pub async fn get_team_by_id(pool: &PgPool, team_id: &str) -> Result<TeamInfo> {
    info!("Fetching team information for team ID: {}", team_id);

    let client = pool.get().await.context("Failed to get DB client for team info")?;

    let query = r#"
        SELECT id, name, display_name, whitelisted_datasets, is_active
        FROM auth.teams
        WHERE id = $1
    "#;

    let row = client.query_opt(query, &[&team_id]).await
        .context("Failed to query team information")?
        .ok_or_else(|| anyhow::anyhow!("No team found for team ID: {}", team_id))?;

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