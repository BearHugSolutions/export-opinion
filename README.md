# Edge Review Dashboard & Export System

This system provides tools for tracking human review progress on entity and service edge visualizations, and exporting the reviewed data to Excel files.

## Features

### 📋 Data Export
- **Excel export** with separate sheets for Organizations and Services
- **Progress Overview tab** with dashboard data embedded in Excel workbooks
- **User opinion-based clustering** that respects human review decisions
- **Parallel processing** for multiple users
- **Timestamped exports** for audit trails

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

### Database Schema
The system expects these table patterns:
- `{schema}.{user_prefix}_entity_edge_visualization`
- `{schema}.{user_prefix}_service_edge_visualization`

With columns:
- `confirmed_status` (VARCHAR)
- `was_reviewed` (BOOLEAN)
- Entity/service ID columns

## Monitoring & Troubleshooting

### Logs
Enable detailed logging:
```bash
RUST_LOG=debug cargo run --bin dashboard
```

## License

GPL License with specifics pending
