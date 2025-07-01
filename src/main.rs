use anyhow::Result;
use chrono::Local;
use log::info;
use std::path::PathBuf;
use futures::future::join_all; // Import join_all for parallel execution

use export_opinion::db_connect;
use export_opinion::dashboard;
use export_opinion::env_loader;
use export_opinion::export_schema;
use export_opinion::reclustering;
use export_opinion::data_fetch;
use export_opinion::excel_writer;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables using your existing loader
    env_loader::load_env();
    env_logger::init(); // Initialize logger

    info!("Starting data export process.");

    // Establish database connection pool using your existing connection logic
    let pool = db_connect::connect().await?;
    info!("Database connection pool established.");

    // Generate dashboard first (before any schema changes)
    let dashboard_path = PathBuf::from("review_dashboard.html");
    info!("Generating review dashboard...");
    match dashboard::generate_dashboard(&pool, &dashboard_path).await {
        Ok(()) => info!("Dashboard generated successfully at: {:?}", dashboard_path),
        Err(e) => {
            log::warn!("Failed to generate dashboard: {}. Continuing with exports...", e);
        }
    }

    // Create the export schema once before processing users
    let schema_client = pool.get().await?;
    export_schema::create_export_schema(&schema_client).await?;
    drop(schema_client); // Release the client back to the pool
    info!("Export schema created/ensured.");

    // Define target users and their prefixes
    // In a real application, these might come from a config file or CLI arguments
    let users = vec![
        ("Hannah", "hannah"),
        ("DrewW", "dreww"),
    ];

    // Create a vector to hold the tasks for each user
    let mut tasks = Vec::new();

    for (username, user_prefix) in users {
        let pool_clone = pool.clone(); // Clone the pool for each task
        let username_clone = username.to_string();
        let user_prefix_clone = user_prefix.to_string();

        // Spawn an asynchronous task for each user's export process
        let task = tokio::spawn(async move {
            info!("Processing export for user: {}", username_clone);

            // Generate a unique timestamp for the export tables and file
            let timestamp_suffix = Local::now().format("%Y%m%d%H%M%S").to_string();
            let export_file_name = format!("{}_export_{}.xlsx", user_prefix_clone, timestamp_suffix);
            let export_file_path = PathBuf::from(export_file_name);

            // Get a client from the pool for table operations
            // These operations are sequential per user, but parallel across users
            let client_for_tables = pool_clone.get().await?;
            
            // Note: Schema creation is now done before spawning tasks
            // Only create timestamped tables here
            export_schema::create_timestamped_tables(&client_for_tables, &user_prefix_clone, &timestamp_suffix).await?;
            drop(client_for_tables); // Release the client back to the pool

            // Run re-clustering for entities
            info!("Running entity re-clustering for user: {}", user_prefix_clone);
            reclustering::run_reclustering(&pool_clone, &user_prefix_clone, &timestamp_suffix, "entity").await?;

            // Run re-clustering for services
            info!("Running service re-clustering for user: {}", user_prefix_clone);
            reclustering::run_reclustering(&pool_clone, &user_prefix_clone, &timestamp_suffix, "service").await?;

            // Fetch organization export data
            info!("Fetching organization data for user: {}", user_prefix_clone);
            let org_data = data_fetch::fetch_organization_export_data(&pool_clone, &user_prefix_clone, &timestamp_suffix).await?;
            info!("Fetched {} organization records.", org_data.len());

            // Fetch service export data
            info!("Fetching service data for user: {}", user_prefix_clone);
            let svc_data = data_fetch::fetch_service_export_data(&pool_clone, &user_prefix_clone, &timestamp_suffix).await?;
            info!("Fetched {} service records.", svc_data.len());

            // Fetch dashboard data for progress overview tab
            info!("Fetching dashboard data for progress overview...");
            let dashboard_data = dashboard::get_dashboard_data(&pool_clone).await.ok(); // Use .ok() to make it optional

            // Write data to Excel file (including progress overview)
            info!("Writing data to Excel file: {:?}", export_file_path);
            excel_writer::write_excel_file(&export_file_path, org_data, svc_data, dashboard_data).await?; 
            info!("Export for user {} completed successfully.", username_clone);

            Ok::<(), anyhow::Error>(()) // Return a Result from the spawned task
        });
        tasks.push(task);
    }

    // Await all spawned tasks. join_all returns a Vec<Result<Result<(), anyhow::Error>, tokio::task::JoinError>>
    let results = join_all(tasks).await;

    // Check for errors from the spawned tasks
    for result in results {
        match result {
            Ok(Ok(())) => {}, // Task completed successfully
            Ok(Err(e)) => {
                eprintln!("Error in user export task: {:?}", e);
                // Depending on requirements, you might want to return an error here
                // or collect all errors and report them at the end.
            },
            Err(e) => {
                eprintln!("Join error in user export task: {:?}", e);
            }
        }
    }

    // Generate dashboard again after exports complete (to reflect any changes)
    info!("Regenerating dashboard after exports...");
    match dashboard::generate_dashboard(&pool, &dashboard_path).await {
        Ok(()) => info!("Final dashboard generated successfully at: {:?}", dashboard_path),
        Err(e) => log::warn!("Failed to regenerate final dashboard: {}", e),
    }

    info!("All exports completed.");
    Ok(())
}