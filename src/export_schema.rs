use anyhow::{Context, Result};
use tokio_postgres::Client;
use log::info;

const TEAM_SCHEMA: &str = "wa211_to_wric";
const EXPORT_SCHEMA: &str = "wa211_to_wric_exports";

/// Creates the dedicated export schema if it does not already exist.
pub async fn create_export_schema(client: &Client) -> Result<()> {
    info!("Ensuring export schema '{}' exists...", EXPORT_SCHEMA);
    let query = format!("CREATE SCHEMA IF NOT EXISTS {};", EXPORT_SCHEMA);
    client.execute(&query, &[]).await
        .context(format!("Failed to create schema {}", EXPORT_SCHEMA))?;
    info!("Schema '{}' ensured.", EXPORT_SCHEMA);
    Ok(())
}

/// Creates and populates the timestamped export tables for a given user.
/// These tables are based on the user's opinionated tables in the team schema.
/// Also removes check constraints that would prevent our reclustering logic from working.
pub async fn create_timestamped_tables(
    client: &Client,
    user_prefix: &str,
    timestamp_suffix: &str,
) -> Result<()> {
    info!("Creating timestamped tables for user '{}' with suffix '{}'...", user_prefix, timestamp_suffix);

    let tables_to_copy = vec![
        "entity_group",
        "entity_group_cluster", 
        "entity_edge_visualization",
        "service_group",
        "service_group_cluster",
        "service_edge_visualization",
    ];

    for table_name in tables_to_copy {
        let source_table_full = format!(r#""{}"."{}_{}""#, TEAM_SCHEMA, user_prefix, table_name);
        let target_table_name = format!("{}_{}_export_{}", user_prefix, table_name, timestamp_suffix);
        let target_table_full = format!(r#""{}"."{}""#, EXPORT_SCHEMA, target_table_name);

        // Drop existing table in export schema to ensure a clean slate for this timestamp
        let drop_query = format!("DROP TABLE IF EXISTS {} CASCADE;", target_table_full);
        client.execute(&drop_query, &[]).await
            .context(format!("Failed to drop table {}", target_table_full))?;

        // Create table structure (LIKE ... INCLUDING ALL)
        let create_query = format!(
            "CREATE TABLE {} (LIKE {} INCLUDING ALL);",
            target_table_full, source_table_full
        );
        client.execute(&create_query, &[]).await
            .context(format!("Failed to create table structure for {}", target_table_full))?;

        // Drop problematic check constraints that prevent our reclustering logic
        if table_name.contains("_group") && !table_name.contains("_group_cluster") {
            // For entity_group and service_group tables, drop constraints that prevent
            // entity_id_1 = entity_id_2. Our reclustering needs self-referencing records
            // for isolated entities, but the original constraints prevent this.
            
            // Query to find all check constraints on this table
            let find_constraints_query = format!(
                r#"
                SELECT conname 
                FROM pg_constraint 
                WHERE conrelid = '{}'::regclass 
                AND contype = 'c'
                AND conname LIKE '%order%' OR conname LIKE '%different%' OR conname LIKE '%check%'
                "#,
                target_table_full
            );
            
            let constraint_rows = client.query(&find_constraints_query, &[]).await
                .unwrap_or_else(|_| vec![]); // If query fails, just continue
            
            for constraint_row in constraint_rows {
                let constraint_name: String = constraint_row.get("conname");
                let drop_constraint_query = format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {};",
                    target_table_full, constraint_name
                );
                
                match client.execute(&drop_constraint_query, &[]).await {
                    Ok(_) => {
                        info!("Dropped constraint '{}' from {}", constraint_name, target_table_full);
                    }
                    Err(e) => {
                        info!("Could not drop constraint '{}' from {}: {}", 
                              constraint_name, target_table_full, e);
                    }
                }
            }
        }

        // Copy data from team schema to the new timestamped table
        let copy_query = format!(
            "INSERT INTO {} SELECT * FROM {};",
            target_table_full, source_table_full
        );
        client.execute(&copy_query, &[]).await
            .context(format!("Failed to copy data to {}", target_table_full))?;

        let count_query = format!("SELECT COUNT(*) FROM {};", target_table_full);
        let count_row = client.query_one(&count_query, &[]).await
            .context(format!("Failed to count rows in {}", target_table_full))?;
        let row_count: i64 = count_row.get(0);
        info!("Copied {} rows to {}.", row_count, target_table_full);
    }

    Ok(())
}