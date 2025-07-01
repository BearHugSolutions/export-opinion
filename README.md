# Edge Review Dashboard & Export System

This system provides tools for tracking human review progress on entity and service edge visualizations, and exporting the reviewed data to Excel files.

## Features

### 📊 Review Dashboard
- **Real-time progress tracking** for edge visualization reviews
- **Per-user statistics** showing pending vs. completed reviews
- **Visual progress bars** and completion percentages  
- **Auto-refreshing HTML dashboard** (updates every 5 minutes)
- **Mobile-responsive design** for viewing on any device

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

### 1. Generate Review Dashboard Only

To quickly check review progress without running exports:

```bash
# Generate dashboard with default filename (review_dashboard.html)
cargo run --bin dashboard

# Generate dashboard with custom filename
cargo run --bin dashboard custom_dashboard.html
```

This creates an HTML file that shows:
- **Overall progress** across all users
- **Per-user breakdown** of entity and service review stats
- **Pending review counts** to track remaining work
- **Completion percentages** and progress bars

### 2. Full Export Process (includes Dashboard)

To run the complete export process:

```bash
cargo run --bin export
```

This will:
1. 🎯 Generate initial dashboard
2. 🔄 Run re-clustering based on user opinions  
3. 📊 Export data to Excel files for each user
4. 🎯 Regenerate dashboard with updated data

### 3. Dashboard Features

The generated HTML dashboard includes:

#### Review Status Tracking
- **Pending Review**: Edges awaiting human review (`PENDING_REVIEW`)
- **Confirmed Match**: Edges marked as true duplicates (`CONFIRMED_MATCH`) 
- **Confirmed Non-Match**: Edges marked as not duplicates (`CONFIRMED_NON_MATCH`)

#### Visual Progress Indicators
- **Progress bars** showing completion percentage
- **Color-coded status**: Green for complete, Orange for pending
- **Grid layout** comparing entity vs. service progress

#### Auto-Refresh
- Dashboard automatically refreshes every 5 minutes
- Timestamp shows last update time
- No manual refresh needed for monitoring

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

### Dashboard
- `review_dashboard.html` - Interactive HTML dashboard
- Auto-refreshes every 5 minutes
- Mobile-responsive design

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

### Customizing Dashboard
Modify the CSS in `dashboard.rs` function `generate_html_dashboard()` to change:
- Colors and styling
- Layout and grid structure  
- Progress bar appearance
- Responsive breakpoints

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

### Common Issues

**Dashboard shows no data**: Check database connection and table names
**Zero review counts**: Verify `confirmed_status` column has expected values
**Dashboard doesn't load**: Check HTML file permissions and browser console

### Performance
- Dashboard generation typically takes < 5 seconds
- Full export process scales with data size (parallel user processing)
- HTML dashboard is lightweight (< 100KB) and loads quickly

## License

[Add your license information here]