use serde::{Deserialize, Serialize};
use serde_json::Value; // For the 'details' jsonb column

// Raw data structs (examples, adjust as needed based on actual queries)
#[derive(Debug, Clone)]
pub struct RawEdgeVisualization {
    pub id: String,
    pub entity_id_1: Option<String>, // Can be entity_id_1 or service_id_1
    pub entity_id_2: Option<String>, // Can be entity_id_2 or service_id_2
    pub service_id_1: Option<String>, // For service edges
    pub service_id_2: Option<String>, // For service edges
    pub confirmed_status: Option<String>,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityEdgeDetails {
    pub contributing_methods: Vec<(String, f64)>, // (method_type, confidence)
    pub total_confidence: f64,
    pub pre_rl_total_confidence: f64,
    pub calculated_edge_weight: f64,
}

// Final export row structs
#[derive(Debug, Serialize)]
pub struct OrganizationExportRow {
    pub contributor: Option<String>,
    pub contributor_id: Option<String>,
    pub entity_id: String,
    pub name: Option<String>,
    pub cluster_confirmed_status: String,
    pub cluster: Option<String>,
    pub has_duplicates: bool,
}

#[derive(Debug, Serialize)]
pub struct ServiceExportRow {
    pub contributor: Option<String>,
    pub contributor_id: Option<String>,
    pub service_id: String,
    pub organization_name: Option<String>,
    pub service_name: Option<String>,
    pub location_name: Option<String>,
    pub full_address: Option<String>,
    pub cluster_confirmed_status: String,
    pub taxonomy_terms: Option<String>, // Comma-separated string
    pub cluster: Option<String>,
    pub has_duplicates: bool,
}