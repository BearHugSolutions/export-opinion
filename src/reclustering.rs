// reclustering.rs
use anyhow::{Context, Result};
use chrono::Local;
use std::collections::{HashMap, HashSet};
use petgraph::graph::{NodeIndex, UnGraph};
use log::{info, debug, warn};
use uuid::Uuid;
use serde_json::{json, Value};
use tokio_postgres::types::ToSql;

use crate::db_connect::PgPool;
use crate::models::{RawEdgeVisualization, EntityEdgeDetails};
use crate::team_utils::{TeamInfo, create_dataset_filter_clause};

const TEAM_SCHEMA: &str = "wa211_to_wric";
const EXPORT_SCHEMA: &str = "wa211_to_wric_exports";

/// Runs the re-clustering logic for either entities or services based on user opinions.
/// This starts with the user's reviewed edges and creates new clusters by filtering out
/// CONFIRMED_NON_MATCH edges and keeping CONFIRMED_MATCH and PENDING_REVIEW edges.
/// Now includes filtering by team's whitelisted datasets.
pub async fn run_reclustering(
    pool: &PgPool,
    user_prefix: &str,
    timestamp_suffix: &str,
    entity_or_service: &str, // "entity" or "service"
    team_info: &TeamInfo,
) -> Result<()> {
    info!("Starting re-clustering for {} for user '{}' with dataset filtering...", entity_or_service, user_prefix);

    let edge_table_name = format!("{}_{}_edge_visualization", user_prefix, entity_or_service);
    let export_edge_table = format!("{}_{}_edge_visualization_export_{}", user_prefix, entity_or_service, timestamp_suffix);
    let export_group_table = format!("{}_{}_group_export_{}", user_prefix, entity_or_service, timestamp_suffix);
    let export_cluster_table = format!("{}_{}_group_cluster_export_{}", user_prefix, entity_or_service, timestamp_suffix);

    let mut client = pool.get().await.context("Failed to get DB client for reclustering")?;

    // 1. Fetch edge data from user's opinionated table
    let query = format!(
        r#"
        SELECT id, {0}_id_1, {0}_id_2, confirmed_status, details, edge_weight
        FROM "{1}"."{2}"
        "#,
        entity_or_service, TEAM_SCHEMA, edge_table_name
    );
    debug!("Fetching edges with query: {}", query);
    let rows = client.query(&query, &[]).await
        .context(format!("Failed to fetch {} edge data for reclustering", entity_or_service))?;

    let mut all_edges: Vec<RawEdgeVisualization> = Vec::new();
    for row in rows {
        all_edges.push(RawEdgeVisualization {
            id: row.get("id"),
            entity_id_1: if entity_or_service == "entity" { row.get(format!("{}_id_1", entity_or_service).as_str()) } else { None },
            entity_id_2: if entity_or_service == "entity" { row.get(format!("{}_id_2", entity_or_service).as_str()) } else { None },
            service_id_1: if entity_or_service == "service" { row.get(format!("{}_id_1", entity_or_service).as_str()) } else { None },
            service_id_2: if entity_or_service == "service" { row.get(format!("{}_id_2", entity_or_service).as_str()) } else { None },
            confirmed_status: row.get("confirmed_status"),
            details: row.get("details"),
        });
    }
    info!("Fetched {} {} edges from user opinions.", all_edges.len(), entity_or_service);

    // 2. Filter edges based on user opinions - keep only valid connections
    let mut graph = UnGraph::<String, EntityEdgeDetails>::new_undirected();
    let mut node_map: HashMap<String, NodeIndex> = HashMap::new();
    let mut valid_edges_for_viz: Vec<(String, String, f64, Value, String)> = Vec::new();

    for edge in &all_edges {
        let id1 = if entity_or_service == "entity" { 
            edge.entity_id_1.clone().unwrap_or_default() 
        } else { 
            edge.service_id_1.clone().unwrap_or_default() 
        };
        let id2 = if entity_or_service == "entity" { 
            edge.entity_id_2.clone().unwrap_or_default() 
        } else { 
            edge.service_id_2.clone().unwrap_or_default() 
        };

        if id1.is_empty() || id2.is_empty() {
            warn!("Skipping edge with empty ID: {:?} - {:?}", id1, id2);
            continue;
        }

        let status = edge.confirmed_status.as_deref().unwrap_or("PENDING_REVIEW");
        
        // Valid connections: CONFIRMED_MATCH or PENDING_REVIEW
        // Invalid connections: CONFIRMED_NON_MATCH (breaks the connection)
        let is_valid_connection = status == "PENDING_REVIEW" || status == "CONFIRMED_MATCH";

        if is_valid_connection {
            // Add nodes to graph if they don't exist
            let node_idx_1 = *node_map.entry(id1.clone()).or_insert_with(|| graph.add_node(id1.clone()));
            let node_idx_2 = *node_map.entry(id2.clone()).or_insert_with(|| graph.add_node(id2.clone()));

            // Extract edge weight and details from the original edge
            let edge_weight = edge.details.as_ref()
                .and_then(|d| d.get("calculated_edge_weight"))
                .and_then(|w| w.as_f64())
                .unwrap_or(1.0); // Default weight if not available

            let edge_details = edge.details.clone().unwrap_or_else(|| {
                json!({
                    "contributing_methods": [],
                    "total_confidence": edge_weight,
                    "pre_rl_total_confidence": edge_weight,
                    "calculated_edge_weight": edge_weight
                })
            });

            // Add edge to graph
            graph.add_edge(node_idx_1, node_idx_2, EntityEdgeDetails {
                contributing_methods: edge_details.get("contributing_methods")
                    .and_then(|m| serde_json::from_value(m.clone()).ok())
                    .unwrap_or_default(),
                total_confidence: edge_details.get("total_confidence")
                    .and_then(|c| c.as_f64())
                    .unwrap_or(edge_weight),
                pre_rl_total_confidence: edge_details.get("pre_rl_total_confidence")
                    .and_then(|c| c.as_f64())
                    .unwrap_or(edge_weight),
                calculated_edge_weight: edge_weight,
            });

            valid_edges_for_viz.push((
                id1.clone(),
                id2.clone(),
                edge_weight,
                edge_details,
                status.to_string(),
            ));
        }
    }

    info!("Built graph with {} nodes and {} valid edges after applying user opinions.", 
          graph.node_count(), graph.edge_count());

    // 3. Get all original entities/services to ensure everything is included, filtered by whitelisted datasets
    let all_original_ids_table = if entity_or_service == "entity" { "entity" } else { "service" };
    let (dataset_filter, filter_params) = create_dataset_filter_clause(
        "t", "source_system", &team_info.whitelisted_datasets, 1
    );
    
    let all_original_ids_query = format!(
        r#"SELECT id FROM public.{} t WHERE {}"#,
        all_original_ids_table, dataset_filter
    );
    
    // Convert filter_params to Vec<&(dyn ToSql + Sync)>
    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = filter_params
        .iter()
        .map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    
    let original_rows = client.query(&all_original_ids_query, &params).await
        .context(format!("Failed to fetch all public {} IDs filtered by whitelisted datasets", entity_or_service))?;

    info!("Found {} original {}s in whitelisted datasets", original_rows.len(), entity_or_service);

    // 4. Identify connected components (new clusters) and handle isolated nodes
    let mut visited = HashSet::new();
    let mut clusters: HashMap<String, HashSet<String>> = HashMap::new();
    let mut node_to_cluster_id: HashMap<String, String> = HashMap::new();

    // First, handle connected components in the graph
    for node_idx in graph.node_indices() {
        let node_id = graph[node_idx].clone();
        if !visited.contains(&node_id) {
            let cluster_id = Uuid::new_v4().to_string();
            let mut stack = vec![node_idx];
            let mut current_cluster_nodes = HashSet::new();

            // DFS to find all connected nodes
            while let Some(current_node_idx) = stack.pop() {
                let current_node_id = graph[current_node_idx].clone();
                if visited.insert(current_node_id.clone()) {
                    current_cluster_nodes.insert(current_node_id.clone());
                    node_to_cluster_id.insert(current_node_id.clone(), cluster_id.clone());
                    
                    for neighbor_node_idx in graph.neighbors(current_node_idx) {
                        let neighbor_node_id = graph[neighbor_node_idx].clone();
                        if !visited.contains(&neighbor_node_id) {
                            stack.push(neighbor_node_idx);
                        }
                    }
                }
            }
            clusters.insert(cluster_id, current_cluster_nodes);
        }
    }

    // Handle isolated nodes (entities/services not in any valid edge, but in whitelisted datasets)
    for row in original_rows {
        let original_id: String = row.get("id");
        if !node_map.contains_key(&original_id) {
            // This entity/service has no valid edges, give it its own cluster
            let cluster_id = Uuid::new_v4().to_string();
            let mut single_node_cluster = HashSet::new();
            single_node_cluster.insert(original_id.clone());
            clusters.insert(cluster_id.clone(), single_node_cluster);
            node_to_cluster_id.insert(original_id, cluster_id);
        }
    }

    info!("Created {} clusters from user opinions (filtered by whitelisted datasets).", clusters.len());

    // 5. Store re-clustered data in timestamped export tables
    let tx = client.transaction().await.context("Failed to start transaction for storing re-clustered data")?;

    // Clear existing data in export tables
    tx.execute(&format!("DELETE FROM \"{}\".\"{}\"", EXPORT_SCHEMA, export_cluster_table), &[]).await?;
    tx.execute(&format!("DELETE FROM \"{}\".\"{}\"", EXPORT_SCHEMA, export_group_table), &[]).await?;
    tx.execute(&format!("DELETE FROM \"{}\".\"{}\"", EXPORT_SCHEMA, export_edge_table), &[]).await?;

    // Insert new cluster records
    let mut cluster_ids_batch: Vec<String> = Vec::new();
    let mut cluster_names_batch: Vec<String> = Vec::new();
    let mut descriptions_batch: Vec<String> = Vec::new();
    let mut entity_counts_batch: Vec<i32> = Vec::new();
    let mut group_counts_batch: Vec<i32> = Vec::new();
    let mut average_coherence_scores_batch: Vec<f64> = Vec::new();

    let group_count_column_name = if entity_or_service == "entity" {
        "group_count"
    } else {
        "service_group_count"
    };

    for (cluster_id, member_ids) in &clusters {
        let cluster_name = format!("{}Cluster-{}", entity_or_service.to_uppercase(), &cluster_id[..8]);
        let description = format!("Re-clustered {} of {} {}s based on user opinions (whitelisted datasets only).", entity_or_service, member_ids.len(), entity_or_service);
        let entity_count = member_ids.len() as i32;
        let group_count = 0; // Will be updated when creating group records
        let average_coherence_score = 0.8; // Placeholder - could calculate based on edge weights

        cluster_ids_batch.push(cluster_id.clone());
        cluster_names_batch.push(cluster_name);
        descriptions_batch.push(description);
        entity_counts_batch.push(entity_count);
        group_counts_batch.push(group_count);
        average_coherence_scores_batch.push(average_coherence_score);
    }

    if !cluster_ids_batch.is_empty() {
        let insert_cluster_batch_query = format!(
            r#"
            INSERT INTO "{}"."{}" (id, name, description, created_at, updated_at, {}_count, {}, average_coherence_score, was_reviewed)
            SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::timestamp[], $5::timestamp[], $6::int4[], $7::int4[], $8::float8[], $9::boolean[])
            "#,
            EXPORT_SCHEMA, export_cluster_table, entity_or_service, group_count_column_name
        );

        let current_timestamp = Local::now().naive_utc();
        let created_at_batch = vec![current_timestamp; cluster_ids_batch.len()];
        let updated_at_batch = vec![current_timestamp; cluster_ids_batch.len()];
        let was_reviewed_batch = vec![true; cluster_ids_batch.len()];

        tx.execute(
            &insert_cluster_batch_query,
            &[
                &cluster_ids_batch as &(dyn ToSql + Sync),
                &cluster_names_batch as &(dyn ToSql + Sync),
                &descriptions_batch as &(dyn ToSql + Sync),
                &created_at_batch as &(dyn ToSql + Sync),
                &updated_at_batch as &(dyn ToSql + Sync),
                &entity_counts_batch as &(dyn ToSql + Sync),
                &group_counts_batch as &(dyn ToSql + Sync),
                &average_coherence_scores_batch as &(dyn ToSql + Sync),
                &was_reviewed_batch as &(dyn ToSql + Sync),
            ],
        ).await.context("Failed to batch insert cluster records")?;
        info!("Inserted {} new {} clusters.", cluster_ids_batch.len(), entity_or_service);
    }

    // Create group records for all entities/services
    let mut group_ids_batch: Vec<String> = Vec::new();
    let mut group_id1s_batch: Vec<String> = Vec::new();
    let mut group_id2s_batch: Vec<String> = Vec::new();
    let mut group_cluster_ids_batch: Vec<String> = Vec::new();
    let mut group_method_types_batch: Vec<String> = Vec::new();

    for (cluster_id, member_ids) in &clusters {
        let member_vec: Vec<String> = member_ids.iter().cloned().collect();
        
        if member_vec.len() == 1 {
            // Single entity cluster - create self-referencing group record
            let entity_id = &member_vec[0];
            group_ids_batch.push(Uuid::new_v4().to_string());
            group_id1s_batch.push(entity_id.clone());
            group_id2s_batch.push(entity_id.clone()); // Self-reference for isolated entities
            group_cluster_ids_batch.push(cluster_id.clone());
            group_method_types_batch.push("USER_REVIEW_ISOLATED".to_string());
        } else {
            // Multi-entity cluster - create pairwise group records
            for i in 0..member_vec.len() {
                for j in (i + 1)..member_vec.len() {
                    group_ids_batch.push(Uuid::new_v4().to_string());
                    group_id1s_batch.push(member_vec[i].clone());
                    group_id2s_batch.push(member_vec[j].clone());
                    group_cluster_ids_batch.push(cluster_id.clone());
                    group_method_types_batch.push("USER_REVIEW_CONNECTED".to_string());
                }
            }
        }
    }

    if !group_ids_batch.is_empty() {
        let insert_group_batch_query = format!(
            r#"
            INSERT INTO "{}"."{}" (id, {}_id_1, {}_id_2, group_cluster_id, method_type, created_at, updated_at, confirmed_status)
            SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::timestamp[], $7::timestamp[], $8::text[])
            "#,
            EXPORT_SCHEMA, export_group_table, entity_or_service, entity_or_service
        );

        let current_timestamp = Local::now().naive_utc();
        let created_at_batch = vec![current_timestamp; group_ids_batch.len()];
        let updated_at_batch = vec![current_timestamp; group_ids_batch.len()];
        let confirmed_status_batch = vec!["CONFIRMED".to_string(); group_ids_batch.len()];

        tx.execute(
            &insert_group_batch_query,
            &[
                &group_ids_batch as &(dyn ToSql + Sync),
                &group_id1s_batch as &(dyn ToSql + Sync),
                &group_id2s_batch as &(dyn ToSql + Sync),
                &group_cluster_ids_batch as &(dyn ToSql + Sync),
                &group_method_types_batch as &(dyn ToSql + Sync),
                &created_at_batch as &(dyn ToSql + Sync),
                &updated_at_batch as &(dyn ToSql + Sync),
                &confirmed_status_batch as &(dyn ToSql + Sync),
            ],
        ).await.context("Failed to batch insert group records")?;
        info!("Inserted {} group records.", group_ids_batch.len());
    }

    // Insert visualization edges for valid connections
    let mut edge_ids_batch: Vec<String> = Vec::new();
    let mut edge_cluster_ids_batch: Vec<String> = Vec::new();
    let mut edge_id1s_batch: Vec<String> = Vec::new();
    let mut edge_id2s_batch: Vec<String> = Vec::new();
    let mut edge_weights_batch: Vec<f64> = Vec::new();
    let mut edge_details_batch: Vec<Value> = Vec::new();
    let mut edge_statuses_batch: Vec<String> = Vec::new();

    let cluster_id_column_name = if entity_or_service == "entity" {
        "cluster_id"
    } else {
        "service_group_cluster_id"
    };

    for (id1, id2, weight, details, status) in valid_edges_for_viz {
        let edge_id = Uuid::new_v4().to_string();
        let cluster_id = node_to_cluster_id.get(&id1).or_else(|| node_to_cluster_id.get(&id2))
            .ok_or_else(|| anyhow::anyhow!("Edge nodes not found in any cluster after reclustering for edge {} - {}", id1, id2))?;
        
        edge_ids_batch.push(edge_id);
        edge_cluster_ids_batch.push(cluster_id.clone());
        edge_id1s_batch.push(id1);
        edge_id2s_batch.push(id2);
        edge_weights_batch.push(weight);
        edge_details_batch.push(details);
        edge_statuses_batch.push(status);
    }

    if !edge_ids_batch.is_empty() {
        let insert_edge_viz_batch_query = format!(
            r#"
            INSERT INTO "{0}"."{1}" (id, {2}, {3}_id_1, {3}_id_2, edge_weight, details, pipeline_run_id, created_at, confirmed_status, was_reviewed)
            SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::float8[], $6::jsonb[], $7::text[], $8::timestamp[], $9::text[], $10::boolean[])
            "#,
            EXPORT_SCHEMA, export_edge_table, cluster_id_column_name, entity_or_service
        );

        let pipeline_run_id_batch = vec!["user_export_pipeline".to_string(); edge_ids_batch.len()];
        let current_timestamp = Local::now().naive_utc();
        let created_at_batch = vec![current_timestamp; edge_ids_batch.len()];
        let was_reviewed_batch = vec![true; edge_ids_batch.len()];

        tx.execute(
            &insert_edge_viz_batch_query,
            &[
                &edge_ids_batch as &(dyn ToSql + Sync),
                &edge_cluster_ids_batch as &(dyn ToSql + Sync),
                &edge_id1s_batch as &(dyn ToSql + Sync),
                &edge_id2s_batch as &(dyn ToSql + Sync),
                &edge_weights_batch as &(dyn ToSql + Sync),
                &edge_details_batch as &(dyn ToSql + Sync),
                &pipeline_run_id_batch as &(dyn ToSql + Sync),
                &created_at_batch as &(dyn ToSql + Sync),
                &edge_statuses_batch as &(dyn ToSql + Sync),
                &was_reviewed_batch as &(dyn ToSql + Sync),
            ],
        ).await.context("Failed to batch insert edge visualization records")?;
        info!("Inserted {} visualization edges into export table.", edge_ids_batch.len());
    }

    tx.commit().await.context("Failed to commit re-clustering transaction")?;

    info!("Re-clustering for {} for user '{}' completed successfully. Created {} clusters (filtered by whitelisted datasets).", 
          entity_or_service, user_prefix, clusters.len());
    Ok(())
}