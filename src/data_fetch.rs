use anyhow::{Context, Result};
use log::{info, debug};
use std::collections::HashMap;
use crate::db_connect::PgPool;
use crate::models::{OrganizationExportRow, ServiceExportRow};
use crate::team_utils::{TeamInfo, create_dataset_filter_clause};

const EXPORT_SCHEMA: &str = "wa211_to_wric_exports";

/// Fetches data for the organization-level export.
/// Now filters by team's whitelisted datasets and uses opinion-based table naming
pub async fn fetch_organization_export_data(
    pool: &PgPool,
    user_prefix: &str,
    opinion_name: &str,
    timestamp_suffix: &str,
    team_info: &TeamInfo,
) -> Result<Vec<OrganizationExportRow>> {
    info!("Fetching organization export data for user '{}' with opinion '{}' filtered by whitelisted datasets...", 
          user_prefix, opinion_name);
    let client = pool.get().await.context("Failed to get DB client for organization data fetch")?;

    // Updated table naming to include opinion: {user_prefix}_{opinion_name}_{table_suffix}_export_{timestamp}
    let cluster_table = format!("{}_{}_entity_group_cluster_export_{}", user_prefix, opinion_name, timestamp_suffix);
    let edge_viz_table = format!("{}_{}_entity_edge_visualization_export_{}", user_prefix, opinion_name, timestamp_suffix);
    let group_table = format!("{}_{}_entity_group_export_{}", user_prefix, opinion_name, timestamp_suffix);

    // Create dataset filter clause for entities
    let (dataset_filter, filter_params) = create_dataset_filter_clause(
        "e", "source_system", &team_info.whitelisted_datasets, 1
    );

    // Query that properly handles user opinion-based clusters with dataset filtering
    let query = format!(
        r#"
        WITH EntityClusters AS (
            -- Get cluster assignment for each entity (filtered by whitelisted datasets)
            SELECT DISTINCT
                e.id AS entity_id,
                eg.group_cluster_id AS cluster_id,
                egc.entity_count AS cluster_entity_count
            FROM
                public.entity e
            LEFT JOIN
                "{0}"."{3}" eg ON (eg.entity_id_1 = e.id OR eg.entity_id_2 = e.id)
            LEFT JOIN
                "{0}"."{1}" egc ON egc.id = eg.group_cluster_id
            WHERE {4}
        ),
        ClusterStatuses AS (
            -- Determine the status of each cluster based on edge visualization records
            SELECT 
                ec.entity_id,
                ec.cluster_id,
                ec.cluster_entity_count,
                CASE 
                    WHEN ec.cluster_id IS NULL THEN 'NO_MATCH'
                    WHEN COUNT(ev.id) = 0 THEN 
                        CASE WHEN ec.cluster_entity_count > 1 THEN 'CONFIRMED' ELSE 'NO_MATCH' END
                    WHEN COUNT(CASE WHEN ev.confirmed_status = 'PENDING_REVIEW' THEN 1 END) > 0 THEN 'PENDING_REVIEW'
                    WHEN COUNT(CASE WHEN ev.confirmed_status = 'CONFIRMED_MATCH' THEN 1 END) > 0 THEN 'CONFIRMED'
                    ELSE 'NO_MATCH'
                END AS cluster_confirmed_status
            FROM 
                EntityClusters ec
            LEFT JOIN
                "{0}"."{2}" ev ON (ev.entity_id_1 = ec.entity_id OR ev.entity_id_2 = ec.entity_id)
                    AND ev.cluster_id = ec.cluster_id
            GROUP BY 
                ec.entity_id, ec.cluster_id, ec.cluster_entity_count
        )
        SELECT
            e.source_system AS contributor,
            e.source_id AS contributor_id,
            e.id AS entity_id,
            e.name AS name,
            COALESCE(cs.cluster_confirmed_status, 'NO_MATCH') AS cluster_confirmed_status,
            cs.cluster_id AS cluster,
            COALESCE((cs.cluster_entity_count > 1), false) AS has_duplicates
        FROM
            public.entity e
        LEFT JOIN
            ClusterStatuses cs ON e.id = cs.entity_id
        WHERE {4}
        ORDER BY
            CASE WHEN cs.cluster_id IS NULL THEN 1 ELSE 0 END, -- NULL clusters last
            cs.cluster_id, 
            e.name
        "#,
        EXPORT_SCHEMA, cluster_table, edge_viz_table, group_table, dataset_filter
    );

    debug!("Fetching organization data with query: {}", query);
    
    // Convert filter_params to Vec<&(dyn ToSql + Sync)>
    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = filter_params
        .iter()
        .map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    let rows = client.query(&query, &params).await
        .context("Failed to fetch organization export data with dataset filtering and opinion-based tables")?;

    let mut data = Vec::new();
    for row in rows {
        data.push(OrganizationExportRow {
            contributor: row.try_get("contributor").unwrap_or(None),
            contributor_id: row.try_get("contributor_id").unwrap_or(None),
            entity_id: row.try_get("entity_id").unwrap(),
            name: row.try_get("name").unwrap_or(None),
            cluster_confirmed_status: row.try_get("cluster_confirmed_status").unwrap(),
            cluster: row.try_get("cluster").unwrap_or(None),
            has_duplicates: row.try_get("has_duplicates").unwrap(),
        });
    }
    
    info!("Fetched {} organization records for export (filtered by whitelisted datasets, opinion: {}).", data.len(), opinion_name);
    Ok(data)
}

/// Fetches data for the service-level export.
/// Now filters by team's whitelisted datasets and uses opinion-based table naming
pub async fn fetch_service_export_data(
    pool: &PgPool,
    user_prefix: &str,
    opinion_name: &str,
    timestamp_suffix: &str,
    team_info: &TeamInfo,
) -> Result<Vec<ServiceExportRow>> {
    info!("Fetching service export data for user '{}' with opinion '{}' filtered by whitelisted datasets...", 
          user_prefix, opinion_name);
    let client = pool.get().await.context("Failed to get DB client for service data fetch")?;

    // Updated table naming to include opinion: {user_prefix}_{opinion_name}_{table_suffix}_export_{timestamp}
    let cluster_table = format!("{}_{}_service_group_cluster_export_{}", user_prefix, opinion_name, timestamp_suffix);
    let edge_viz_table = format!("{}_{}_service_edge_visualization_export_{}", user_prefix, opinion_name, timestamp_suffix);
    let group_table = format!("{}_{}_service_group_export_{}", user_prefix, opinion_name, timestamp_suffix);

    // The service edge visualization table uses 'service_group_cluster_id'
    let service_cluster_id_column_name = "service_group_cluster_id";

    // Create dataset filter clause for services
    let (dataset_filter, filter_params) = create_dataset_filter_clause(
        "s", "source_system", &team_info.whitelisted_datasets, 1
    );

    // Query that properly handles user opinion-based service clusters with taxonomy data and dataset filtering
    let query = format!(
        r#"
        WITH ServiceClusters AS (
            -- Get cluster assignment for each service (filtered by whitelisted datasets)
            SELECT DISTINCT
                s.id AS service_id,
                sg.group_cluster_id AS cluster_id,
                sgc.service_count AS cluster_service_count
            FROM
                public.service s
            LEFT JOIN
                "{0}"."{3}" sg ON (sg.service_id_1 = s.id OR sg.service_id_2 = s.id)
            LEFT JOIN
                "{0}"."{1}" sgc ON sgc.id = sg.group_cluster_id
            WHERE {5}
        ),
        ClusterStatuses AS (
            -- Determine the status of each service cluster based on edge visualization records
            SELECT 
                sc.service_id,
                sc.cluster_id,
                sc.cluster_service_count,
                CASE 
                    WHEN sc.cluster_id IS NULL THEN 'NO_MATCH'
                    WHEN COUNT(sv.id) = 0 THEN 
                        CASE WHEN sc.cluster_service_count > 1 THEN 'CONFIRMED' ELSE 'NO_MATCH' END
                    WHEN COUNT(CASE WHEN sv.confirmed_status = 'PENDING_REVIEW' THEN 1 END) > 0 THEN 'PENDING_REVIEW'
                    WHEN COUNT(CASE WHEN sv.confirmed_status = 'CONFIRMED_MATCH' THEN 1 END) > 0 THEN 'CONFIRMED'
                    ELSE 'NO_MATCH'
                END AS cluster_confirmed_status
            FROM 
                ServiceClusters sc
            LEFT JOIN
                "{0}"."{2}" sv ON (sv.service_id_1 = sc.service_id OR sv.service_id_2 = sc.service_id)
                    AND sv.{4} = sc.cluster_id
            GROUP BY 
                sc.service_id, sc.cluster_id, sc.cluster_service_count
        )
        SELECT
            s.contributor_id AS contributor,
            s.source_system AS contributor_id,
            s.id AS service_id,
            o.name AS organization_name,
            s.name AS service_name,
            (
                SELECT l.name
                FROM public.service_at_location sal
                JOIN public.location l ON sal.location_id = l.id
                WHERE sal.service_id = s.id
                ORDER BY sal.id
                LIMIT 1
            ) AS location_name,
            (
                SELECT 
                    a.address_1 || 
                    COALESCE(', ' || a.address_2, '') || 
                    ', ' || a.city || 
                    ', ' || a.state_province || 
                    ' ' || a.postal_code || 
                    ', ' || a.country
                FROM public.address a
                JOIN public.service_at_location sal ON a.location_id = sal.location_id
                WHERE sal.service_id = s.id
                ORDER BY sal.id, a.id
                LIMIT 1
            ) AS full_address,
            COALESCE(cs.cluster_confirmed_status, 'NO_MATCH') AS cluster_confirmed_status,
            t.id AS taxonomy_id,
            t.term AS taxonomy_term,
            t.description AS taxonomy_description,
            t.taxonomy AS taxonomy_category,
            cs.cluster_id AS cluster,
            COALESCE((cs.cluster_service_count > 1), false) AS has_duplicates
        FROM
            public.service s
        LEFT JOIN 
            public.organization o ON s.organization_id = o.id
        LEFT JOIN
            ClusterStatuses cs ON s.id = cs.service_id
        LEFT JOIN 
            public.service_taxonomy st ON s.id = st.service_id
        LEFT JOIN 
            public.taxonomy_term t ON st.taxonomy_term_id = t.id
        WHERE {5}
        ORDER BY
            CASE WHEN cs.cluster_id IS NULL THEN 1 ELSE 0 END, -- NULL clusters last
            cs.cluster_id, 
            s.name,
            t.term
        "#,
        EXPORT_SCHEMA, cluster_table, edge_viz_table, group_table, service_cluster_id_column_name, dataset_filter
    );

    debug!("Fetching service data with query: {}", query);
    
    // Convert filter_params to Vec<&(dyn ToSql + Sync)>
    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = filter_params
        .iter()
        .map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    let rows = client.query(&query, &params).await
        .context("Failed to fetch service export data with dataset filtering and opinion-based tables")?;

    // Group rows by service_id to handle multiple taxonomy terms per service
    let mut service_map: HashMap<String, Vec<tokio_postgres::Row>> = HashMap::new();

    for row in rows {
        let service_id: String = row.try_get("service_id").unwrap();
        service_map.entry(service_id).or_insert_with(Vec::new).push(row);
    }

    debug!("Grouped {} services with taxonomy data (filtered by whitelisted datasets, opinion: {})", service_map.len(), opinion_name);

    let mut data = Vec::new();
    for (_service_id, service_rows) in service_map {
        let first_row = &service_rows[0];
        
        // Collect taxonomy terms from all rows for this service
        let taxonomy_terms: Vec<String> = service_rows
            .iter()
            .filter_map(|row| {
                let taxonomy_term: Option<String> = row.try_get("taxonomy_term").unwrap_or(None);
                taxonomy_term
            })
            .collect();
        
        // Sort taxonomy terms for consistent output
        let mut sorted_taxonomy_terms = taxonomy_terms;
        sorted_taxonomy_terms.sort();
        
        // Join taxonomy terms with comma separation
        let taxonomy_terms_string = if sorted_taxonomy_terms.is_empty() {
            None
        } else {
            Some(sorted_taxonomy_terms.join(", "))
        };
        
        data.push(ServiceExportRow {
            contributor: first_row.try_get("contributor").unwrap_or(None),
            contributor_id: first_row.try_get("contributor_id").unwrap_or(None),
            service_id: first_row.try_get("service_id").unwrap(),
            organization_name: first_row.try_get("organization_name").unwrap_or(None),
            service_name: first_row.try_get("service_name").unwrap_or(None),
            location_name: first_row.try_get("location_name").unwrap_or(None),
            full_address: first_row.try_get("full_address").unwrap_or(None),
            cluster_confirmed_status: first_row.try_get("cluster_confirmed_status").unwrap(),
            taxonomy_terms: taxonomy_terms_string,
            cluster: first_row.try_get("cluster").unwrap_or(None),
            has_duplicates: first_row.try_get("has_duplicates").unwrap(),
        });
    }
    
    // Sort the final data for consistent output
    data.sort_by(|a, b| {
        // Sort by cluster (None last), then by service name
        match (&a.cluster, &b.cluster) {
            (None, None) => a.service_name.cmp(&b.service_name),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(cluster_a), Some(cluster_b)) => {
                cluster_a.cmp(cluster_b).then_with(|| a.service_name.cmp(&b.service_name))
            }
        }
    });
    
    info!("Fetched {} service records for export (filtered by whitelisted datasets, opinion: {}).", data.len(), opinion_name);
    Ok(data)
}