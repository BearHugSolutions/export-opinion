use anyhow::Result;
use log::info;
use std::path::PathBuf;
use std::env;

use export_opinion::db_connect;
use export_opinion::dashboard;
use export_opinion::env_loader;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables using your existing loader
    env_loader::load_env();
    env_logger::init(); // Initialize logger

    info!("Starting dashboard generation...");

    // Get output path from command line args or use default
    let args: Vec<String> = env::args().collect();
    let output_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("review_dashboard.html")
    };

    // Establish database connection pool using your existing connection logic
    let pool = db_connect::connect().await?;
    info!("Database connection pool established.");

    // Generate dashboard
    dashboard::generate_dashboard(&pool, &output_path).await?;
    
    info!("Dashboard generation completed successfully!");
    info!("Dashboard available at: {:?}", output_path);
    
    // Print a helpful message
    println!("\n🎉 Dashboard generated successfully!");
    println!("📊 Open {:?} in your web browser to view the review progress", output_path);
    println!("🔄 The dashboard will auto-refresh every 5 minutes");
    
    Ok(())
}