use anyhow::Result;
use chrono::Local;
use log::info;
use std::path::PathBuf;
use dialoguer::{theme::ColorfulTheme, Select};

use export_opinion::db_connect;
use export_opinion::dashboard;
use export_opinion::env_loader;
use export_opinion::export_schema;
use export_opinion::reclustering;
use export_opinion::data_fetch;
use export_opinion::excel_writer;
use export_opinion::team_utils::{self, TeamInfo, UserInfo, OpinionInfo};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables using your existing loader
    env_loader::load_env();
    env_logger::init(); // Initialize logger

    info!("Starting interactive data export process.");

    // Establish database connection pool using your existing connection logic
    let pool = db_connect::connect().await?;
    info!("Database connection pool established.");

    // Interactive CLI workflow
    let (selected_team, selected_user, selected_opinion) = run_interactive_selection(&pool).await?;
    
    info!(
        "Selected export configuration: Team='{}', User='{}', Opinion='{}'",
        selected_team.display_name, selected_user.username, selected_opinion.name
    );

    // Create the export schema once before processing
    let schema_client = pool.get().await?;
    export_schema::create_export_schema(&schema_client).await?;
    drop(schema_client); // Release the client back to the pool
    info!("Export schema created/ensured.");

    // Generate a unique timestamp for the export tables and file
    let timestamp_suffix = Local::now().format("%Y%m%d%H%M%S").to_string();
    let user_prefix = selected_user.user_opinion_prefix.as_deref()
        .ok_or_else(|| anyhow::anyhow!("User has no opinion prefix set"))?;
    
    let export_file_name = format!("{}_{}_export_{}.xlsx", user_prefix, selected_opinion.name, timestamp_suffix);
    let export_file_path = PathBuf::from(export_file_name);

    info!("Processing export for user: {} with opinion: {} (team: {}, datasets: {:?})", 
          selected_user.username, selected_opinion.name, selected_team.name, selected_team.whitelisted_datasets);

    // Get a client from the pool for table operations
    let client_for_tables = pool.get().await?;
    
    // Create timestamped tables with opinion-specific naming
    export_schema::create_timestamped_tables(&client_for_tables, user_prefix, &selected_opinion.name, &timestamp_suffix).await?;
    drop(client_for_tables); // Release the client back to the pool

    // Run re-clustering for entities with dataset filtering
    info!("Running entity re-clustering for user: {} with opinion: {} (filtered by whitelisted datasets)", user_prefix, selected_opinion.name);
    reclustering::run_reclustering(&pool, user_prefix, &selected_opinion.name, &timestamp_suffix, "entity", &selected_team).await?;

    // Run re-clustering for services with dataset filtering
    info!("Running service re-clustering for user: {} with opinion: {} (filtered by whitelisted datasets)", user_prefix, selected_opinion.name);
    reclustering::run_reclustering(&pool, user_prefix, &selected_opinion.name, &timestamp_suffix, "service", &selected_team).await?;

    // Fetch organization export data with dataset filtering
    info!("Fetching organization data for user: {} with opinion: {} (filtered by whitelisted datasets)", user_prefix, selected_opinion.name);
    let org_data = data_fetch::fetch_organization_export_data(&pool, user_prefix, &selected_opinion.name, &timestamp_suffix, &selected_team).await?;
    info!("Fetched {} organization records (filtered by whitelisted datasets).", org_data.len());

    // Fetch service export data with dataset filtering
    info!("Fetching service data for user: {} with opinion: {} (filtered by whitelisted datasets)", user_prefix, selected_opinion.name);
    let svc_data = data_fetch::fetch_service_export_data(&pool, user_prefix, &selected_opinion.name, &timestamp_suffix, &selected_team).await?;
    info!("Fetched {} service records (filtered by whitelisted datasets).", svc_data.len());

    // Fetch dashboard data for progress overview tab with dataset filtering
    info!("Fetching dashboard data for progress overview (filtered by whitelisted datasets)...");
    let dashboard_data = dashboard::get_dashboard_data(&pool, &selected_user, &selected_opinion, &selected_team).await.ok(); // Use .ok() to make it optional

    // Write data to Excel file (including progress overview)
    info!("Writing data to Excel file: {:?}", export_file_path);
    excel_writer::write_excel_file(&export_file_path, org_data, svc_data, dashboard_data).await?; 
    info!("Export for user {} with opinion {} completed successfully (filtered by team's whitelisted datasets).", selected_user.username, selected_opinion.name);

    Ok(())
}

/// Runs the interactive selection process for team, user, and opinion
async fn run_interactive_selection(pool: &db_connect::PgPool) -> Result<(TeamInfo, UserInfo, OpinionInfo)> {
    let theme = ColorfulTheme::default();
    
    // Step 1: Team Selection
    println!("\n🏢 Select a team:");
    let teams = team_utils::get_all_teams(pool).await?;
    
    if teams.is_empty() {
        return Err(anyhow::anyhow!("No teams found in the database"));
    }
    
    let team_options: Vec<String> = teams.iter()
        .map(|t| format!("{} ({})", t.display_name, t.name))
        .collect();
    
    let team_selection = Select::with_theme(&theme)
        .with_prompt("Choose a team")
        .default(0)
        .items(&team_options)
        .interact()?;
    
    let selected_team = teams[team_selection].clone();
    println!("✅ Selected team: {}", selected_team.display_name);
    
    // Step 2: User Selection
    println!("\n👤 Select a user:");
    let users = team_utils::get_users_for_team(pool, &selected_team.id).await?;
    
    if users.is_empty() {
        return Err(anyhow::anyhow!("No users found for team: {}", selected_team.display_name));
    }
    
    let user_options: Vec<String> = users.iter()
        .map(|u| {
            let prefix = u.user_opinion_prefix.as_deref().unwrap_or("no prefix");
            format!("{} ({})", u.username, prefix)
        })
        .collect();
    
    let user_selection = Select::with_theme(&theme)
        .with_prompt("Choose a user")
        .default(0)
        .items(&user_options)
        .interact()?;
    
    let selected_user = users[user_selection].clone();
    println!("✅ Selected user: {}", selected_user.username);
    
    // Step 3: Opinion Selection
    println!("\n💭 Select an opinion:");
    let opinions = team_utils::get_opinions_for_user(pool, &selected_user.id).await?;
    
    if opinions.is_empty() {
        return Err(anyhow::anyhow!("No opinions found for user: {}", selected_user.username));
    }
    
    let opinion_options: Vec<String> = opinions.iter()
        .map(|o| {
            if o.user_id == selected_user.id {
                format!("opinion owner: {} - opinion name: {}", o.owner_username, o.name)
            } else {
                format!("opinion owner: {} - opinion name: {} (shared)", o.owner_username, o.name)
            }
        })
        .collect();
    
    let opinion_selection = Select::with_theme(&theme)
        .with_prompt("Choose an opinion")
        .default(0)
        .items(&opinion_options)
        .interact()?;
    
    let selected_opinion = opinions[opinion_selection].clone();
    println!("✅ Selected opinion: {} (owner: {})", selected_opinion.name, selected_opinion.owner_username);
    
    Ok((selected_team, selected_user, selected_opinion))
}