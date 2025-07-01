# Edge Review Dashboard & Export System

This system provides tools for tracking human review progress on entity and service edge visualizations, and exporting the reviewed data to Excel files with **opinion-based reclustering**.

## Features

### 📋 Data Export
- **Excel export** with separate sheets for Organizations and Services
- **Progress Overview tab** with dashboard data embedded in Excel workbooks
- **User opinion-based clustering** that respects human review decisions
- **Parallel processing** for multiple users
- **Timestamped exports** for audit trails

### 🔄 Intelligent Reclustering
- **Human-in-the-loop clustering** that incorporates reviewer decisions
- **Graph-based connected components** algorithm for cluster formation
- **Isolated entity handling** for entities with no valid connections
- **Edge filtering** based on confirmed match/non-match status

## Getting Started

### Prerequisites
- Rust 1.70+ installed
- PostgreSQL database with edge visualization tables
- Environment variables configured (see `.env.example`)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd edge-review-exporter

# Build the applications
cargo build --release
```

### Environment Setup

Create a `.env` file with your database connection details:

```env
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_DB=dataplatform
POSTGRES_USER=postgres
POSTGRES_PASSWORD=your_password_here
RUST_LOG=info
```

## Usage

### 1. Full Export Process (includes Dashboard)

To run the complete export process:

```bash
cargo run --bin export
```

This will:
1. 🎯 Generate initial dashboard
2. 🔄 Run re-clustering based on user opinions  
3. 📊 Export data to Excel files for each user
4. 🎯 Regenerate dashboard with updated data

## 🧠 Reclustering Logic Deep Dive

The reclustering system is the core innovation of this tool. It takes machine-generated entity/service similarity predictions and refines them based on human reviewer decisions.

### Why Reclustering?

**Problem**: Initial clustering algorithms are probabilistic and make mistakes. Humans review edge connections and mark them as valid duplicates (`CONFIRMED_MATCH`) or false positives (`CONFIRMED_NON_MATCH`).

**Solution**: Use human feedback to rebuild clusters that respect reviewer decisions, ensuring the final export reflects human intelligence rather than just algorithmic predictions.

### Reclustering Algorithm

#### Step 1: Edge Opinion Collection
```
Input: User's edge visualization table with confirmed_status values
- PENDING_REVIEW: Awaiting human review
- CONFIRMED_MATCH: Human confirmed these are duplicates  
- CONFIRMED_NON_MATCH: Human confirmed these are NOT duplicates
```

#### Step 2: Edge Filtering
The algorithm respects human decisions by:
- **Keeping** edges marked as `CONFIRMED_MATCH` (human says "yes, these are duplicates")
- **Keeping** edges marked as `PENDING_REVIEW` (neutral, algorithm prediction stands)
- **Removing** edges marked as `CONFIRMED_NON_MATCH` (human says "no, these are different entities")

```rust
// Pseudo-code logic
if status == "CONFIRMED_NON_MATCH" {
    // Break this connection - don't include in clustering graph
    skip_edge();
} else {
    // Keep this connection for clustering
    add_to_graph(entity1, entity2, edge_weight);
}
```

#### Step 3: Graph Construction
- Build an undirected graph where:
  - **Nodes** = entities/services
  - **Edges** = valid connections (CONFIRMED_MATCH or PENDING_REVIEW)
  - **Weights** = original confidence scores from ML algorithms

#### Step 4: Connected Components Analysis
Uses depth-first search to find connected components:
```
For each unvisited node:
    Start new cluster
    DFS traverse all connected nodes
    Add all connected nodes to same cluster
```

#### Step 5: Isolated Entity Handling
Entities with no valid edges get their own singleton clusters:
- **Why**: An entity might have had edges that were all marked `CONFIRMED_NON_MATCH`
- **Result**: These become "unique" entities in the final export

#### Step 6: Export Table Generation
Creates new timestamped tables with reclustered data:
- `{user}_entity_group_cluster_export_{timestamp}`: Cluster definitions
- `{user}_entity_group_export_{timestamp}`: Pairwise group relationships  
- `{user}_entity_edge_visualization_export_{timestamp}`: Valid edges only

### Reclustering Example

**Before Reclustering:**
```
Original Cluster A: [Entity1, Entity2, Entity3]
├─ Edge1-2: CONFIRMED_MATCH ✓
├─ Edge1-3: CONFIRMED_NON_MATCH ✗ 
└─ Edge2-3: PENDING_REVIEW ○
```

**After Reclustering:**
```
New Cluster A: [Entity1, Entity2] (connected via confirmed match)
New Cluster B: [Entity3] (isolated due to non-match with Entity1)
```

The human decision to mark Edge1-3 as non-match broke Entity3 away from the cluster.

## Understanding the Data

### Edge Visualization Tables
The system tracks human reviews in tables like:
- `{user_prefix}_entity_edge_visualization` 
- `{user_prefix}_service_edge_visualization`

### Review Statuses
- **`PENDING_REVIEW`**: Requires human review (treated as valid connection)
- **`CONFIRMED_MATCH`**: Human confirmed these are duplicates (strong valid connection)
- **`CONFIRMED_NON_MATCH`**: Human confirmed these are NOT duplicates (connection broken)

### Cluster Status Logic
The final `cluster_confirmed_status` in exports follows this priority:
1. **PENDING_REVIEW**: Any edge in cluster is pending → entire cluster pending
2. **CONFIRMED**: All edges confirmed OR multi-entity cluster with no edges  
3. **NO_MATCH**: Single entity with no edges or no cluster assigned

### Export Data Fields

#### Organizations Sheet
- `cluster_confirmed_status`: Status of the entire cluster this entity belongs to
- `cluster`: UUID of the reclustered group
- `has_duplicates`: Boolean indicating if cluster contains multiple entities

#### Services Sheet  
- `cluster_confirmed_status`: Status of the entire cluster this service belongs to
- `cluster`: UUID of the reclustered group
- `has_duplicates`: Boolean indicating if cluster contains multiple services
- `taxonomy_terms`: Comma-separated taxonomy classifications

## File Outputs

### Excel Exports  
- `{user_prefix}_export_{timestamp}.xlsx` files
- **Progress Overview sheet**: 
  - Overall progress summary across all users
  - User-by-user breakdown of entity and service review stats
  - Completion percentages and pending review counts
  - Timestamp of export generation
- **Organizations sheet**: Entity-level data with cluster assignments
- **Services sheet**: Service-level data with taxonomy terms and addresses

## Development

### Adding New Users
Update the user list in `main.rs` and `dashboard.rs`:

```rust
let users = vec![
    ("Hannah", "hannah"),
    ("DrewW", "dreww"),
    ("NewUser", "newuser"), // Add here
];
```

### Database Schema Requirements
The system expects these table patterns:
- `{schema}.{user_prefix}_entity_edge_visualization`
- `{schema}.{user_prefix}_service_edge_visualization`

With required columns:
- `confirmed_status` (VARCHAR): PENDING_REVIEW, CONFIRMED_MATCH, CONFIRMED_NON_MATCH
- `was_reviewed` (BOOLEAN): Tracking flag
- `{entity_or_service}_id_1` (VARCHAR): First entity/service ID
- `{entity_or_service}_id_2` (VARCHAR): Second entity/service ID  
- `details` (JSONB): Edge metadata and confidence scores
- `edge_weight` (FLOAT): ML algorithm confidence

### Extending Reclustering Logic

To modify clustering behavior, edit `reclustering.rs`:

```rust
// Change edge filtering criteria
let is_valid_connection = match status {
    "CONFIRMED_MATCH" => true,
    "PENDING_REVIEW" => true,  // Could change this to false for stricter clustering
    "CONFIRMED_NON_MATCH" => false,
    _ => false, // Handle new status types
};
```

## Monitoring & Troubleshooting

### Logs
Enable detailed logging:
```bash
RUST_LOG=debug cargo run --bin export
```

## License

GPL License with specifics pending
