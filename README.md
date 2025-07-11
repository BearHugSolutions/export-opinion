# Export System for Edge Review Data

This system exports human-reviewed entity and service edge visualization data to Excel files with embedded progress tracking.

## Features

### 📋 Data Export
- **Excel export** with separate sheets for Organizations and Services
- **Progress Overview tab** with comprehensive dashboard data embedded in Excel workbooks
- **User opinion-based clustering** that respects human review decisions
- **Parallel processing** for multiple users
- **Timestamped exports** for audit trails

### 📊 Progress Tracking
- **Real-time progress statistics** for edge visualization reviews
- **Per-user breakdown** showing pending vs. completed reviews
- **Visual completion percentages** and review counts
- **Overall progress summary** across all users and record types

## Getting Started

### Prerequisites
- Rust 1.70+ installed
- PostgreSQL database with edge visualization tables
- Environment variables configured (see `.env.example`)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd export-opinion

# Build the application
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

### Run Export Process

To run the complete export process:

```bash
cargo run --bin export
```

This will:
1. 🔄 Run re-clustering based on user opinions  
2. 📊 Export data to Excel files for each user
3. 📈 Include Progress Overview tab with dashboard data

Each user gets a timestamped Excel file: `{user_prefix}_export_{timestamp}.xlsx`

## Understanding the Data

### Edge Visualization Tables
The system tracks human reviews in tables like:
- `{user_prefix}_entity_edge_visualization` 
- `{user_prefix}_service_edge_visualization`

### Review Statuses
- **`PENDING_REVIEW`**: Requires human review
- **`CONFIRMED_MATCH`**: Human confirmed these are duplicates
- **`CONFIRMED_NON_MATCH`**: Human confirmed these are NOT duplicates

### Cluster Status Logic
The final `cluster_confirmed_status` in exports follows this priority:
1. **PENDING_REVIEW**: Any edge in cluster is pending → entire cluster pending
2. **CONFIRMED**: All edges confirmed OR multi-entity cluster with no edges
3. **NO_MATCH**: Single entity with no edges or no cluster assigned

## Excel File Structure

Each export contains three sheets:

### 1. Progress Overview Sheet
- **Overall Progress Summary**:
  - Total pending and reviewed counts across entity and service records
  - Overall completion percentage
  - Cross-user statistics
- **User Breakdown**:
  - Per-user statistics for both entity and service reviews
  - Individual completion percentages
  - Detailed pending/confirmed/non-match counts
- **Timestamp**: When the export was generated

### 2. Organizations Sheet
Entity-level data including:
- Contributor information
- Entity IDs and names
- Cluster assignments and confirmation status
- Duplicate detection flags

### 3. Services Sheet  
Service-level data including:
- Service and organization details
- Location and address information
- Taxonomy term classifications
- Cluster assignments and confirmation status
- Duplicate detection flags

## Progress Tracking Details

### Review Status Tracking
- **Pending Review**: Edges awaiting human review (`PENDING_REVIEW`)
- **Confirmed Match**: Edges marked as true duplicates (`CONFIRMED_MATCH`) 
- **Confirmed Non-Match**: Edges marked as not duplicates (`CONFIRMED_NON_MATCH`)

### Completion Metrics
- **Review Percentage**: (Confirmed Match + Confirmed Non-Match) / Total Records
- **Total Records**: All edges requiring review
- **Reviewed Count**: Edges with human decisions (excluding pending)

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

### Database Schema
The system expects these table patterns:
- `{schema}.{user_prefix}_entity_edge_visualization`
- `{schema}.{user_prefix}_service_edge_visualization`

With columns:
- `confirmed_status` (VARCHAR)
- `was_reviewed` (BOOLEAN)
- Entity/service ID columns

### Re-clustering Logic
The system:
1. **Fetches user opinions** from edge visualization tables
2. **Filters edges** based on review status:
   - Keeps: `CONFIRMED_MATCH` and `PENDING_REVIEW` edges
   - Removes: `CONFIRMED_NON_MATCH` edges (breaks connections)
3. **Creates new clusters** using connected components
4. **Handles isolated entities** with self-referencing cluster records
5. **Exports timestamped tables** with user-opinion-based clustering

## Monitoring & Troubleshooting

### Logs
Enable detailed logging:
```bash
RUST_LOG=debug cargo run --bin export
```

### Common Issues

**Empty Progress Overview**: Check database connection and table names
**Zero review counts**: Verify `confirmed_status` column has expected values  
**Export fails**: Check database permissions and disk space for Excel files
**Missing users**: Verify user prefixes match database table naming

### Performance
- Export process scales with data size (parallel user processing)
- Progress Overview generation typically takes < 5 seconds
- Re-clustering performance depends on edge count and cluster size
- Excel file generation is optimized for large datasets

## File Outputs

### Excel Exports  
- `{user_prefix}_export_{timestamp}.xlsx` files per user
- **Progress Overview sheet**: Comprehensive review statistics and completion tracking
- **Organizations sheet**: Entity-level data with cluster assignments
- **Services sheet**: Service-level data with taxonomy terms and addresses

### Timestamped Tables
The system creates export schema tables with timestamps:
- `{user_prefix}_entity_group_cluster_export_{timestamp}`
- `{user_prefix}_service_group_cluster_export_{timestamp}` 
- `{user_prefix}_entity_edge_visualization_export_{timestamp}`
- `{user_prefix}_service_edge_visualization_export_{timestamp}`

## License

[Add your license information here]