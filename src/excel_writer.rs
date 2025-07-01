use anyhow::Result;
use rust_xlsxwriter::{Workbook, FormatAlign, Worksheet, Format};
use std::path::Path;
use log::info;
use chrono;

use crate::models::{OrganizationExportRow, ServiceExportRow};
use crate::dashboard::{UserDashboard, ReviewStats};

/// Writes the extracted organization and service data to an Excel file with multiple sheets.
pub async fn write_excel_file(
    file_path: &Path,
    org_data: Vec<OrganizationExportRow>,
    svc_data: Vec<ServiceExportRow>,
    dashboard_data: Option<Vec<UserDashboard>>,
) -> Result<()> {
    info!("Initializing Excel workbook for file: {:?}", file_path);
    let mut workbook = Workbook::new();

    // Add "Progress Overview" sheet first if dashboard data is provided
    if let Some(progress_data) = dashboard_data {
        let progress_sheet = workbook.add_worksheet();
        write_progress_overview_sheet(progress_sheet, progress_data)?;
    }

    // Add "Organizations" sheet
    let org_sheet = workbook.add_worksheet();
    write_organization_sheet(org_sheet, org_data)?;

    // Add "Services" sheet
    let svc_sheet = workbook.add_worksheet();
    write_service_sheet(svc_sheet, svc_data)?;

    info!("Saving Excel workbook...");
    workbook.save(file_path)?;
    info!("Excel file saved successfully to {:?}", file_path);
    Ok(())
}

/// Helper function to write data to the "Organizations" sheet.
fn write_organization_sheet(sheet: &mut Worksheet, data: Vec<OrganizationExportRow>) -> Result<()> {
    sheet.set_name("Organizations")?;

    // Define headers
    let headers = vec![
        "contributor",
        "contributor_id",
        "entity_id",
        "name",
        "cluster_confirmed_status",
        "cluster",
        "has_duplicates",
    ];

    // Write headers
    for (col_num, header) in headers.iter().enumerate() {
        sheet.write_string(0, col_num as u16, *header)?;
    }

    // Write data rows
    for (row_num, row_data) in data.iter().enumerate() {
        let current_row = (row_num + 1) as u32; // +1 for header row
        sheet.write_string(current_row, 0, row_data.contributor.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 1, row_data.contributor_id.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 2, &row_data.entity_id)?;
        sheet.write_string(current_row, 3, row_data.name.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 4, &row_data.cluster_confirmed_status)?;
        sheet.write_string(current_row, 5, row_data.cluster.as_deref().unwrap_or(""))?;
        sheet.write_boolean(current_row, 6, row_data.has_duplicates)?;
    }
    info!("'Organizations' sheet written with {} rows.", data.len());
    Ok(())
}

/// Helper function to write data to the "Services" sheet.
fn write_service_sheet(sheet: &mut Worksheet, data: Vec<ServiceExportRow>) -> Result<()> {
    sheet.set_name("Services")?;

    // Define headers
    let headers = vec![
        "contributor",
        "contributor_id",
        "service_id",
        "organization_name",
        "service_name",
        "location_name",
        "full_address",
        "cluster_confirmed_status",
        "taxonomy_terms",
        "cluster",
        "has_duplicates",
    ];

    // Write headers
    for (col_num, header) in headers.iter().enumerate() {
        sheet.write_string(0, col_num as u16, *header)?;
    }

    // Write data rows
    for (row_num, row_data) in data.iter().enumerate() {
        let current_row = (row_num + 1) as u32; // +1 for header row
        sheet.write_string(current_row, 0, row_data.contributor.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 1, row_data.contributor_id.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 2, &row_data.service_id)?;
        sheet.write_string(current_row, 3, row_data.organization_name.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 4, row_data.service_name.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 5, row_data.location_name.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 6, row_data.full_address.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 7, &row_data.cluster_confirmed_status)?;
        sheet.write_string(current_row, 8, row_data.taxonomy_terms.as_deref().unwrap_or(""))?;
        sheet.write_string(current_row, 9, row_data.cluster.as_deref().unwrap_or(""))?;
        sheet.write_boolean(current_row, 10, row_data.has_duplicates)?;
    }
    info!("'Services' sheet written with {} rows.", data.len());
    Ok(())
}

/// Helper function to write dashboard data to the "Progress Overview" sheet.
fn write_progress_overview_sheet(sheet: &mut Worksheet, data: Vec<UserDashboard>) -> Result<()> {
    sheet.set_name("Progress Overview")?;

    // Set column widths for better readability
    sheet.set_column_width(0, 20)?; // User/Metric column
    sheet.set_column_width(1, 15)?; // User Prefix column  
    sheet.set_column_width(2, 15)?; // Record Type column
    sheet.set_column_width(3, 15)?; // Pending Review column
    sheet.set_column_width(4, 15)?; // Confirmed Match column
    sheet.set_column_width(5, 18)?; // Confirmed Non-Match column
    sheet.set_column_width(6, 15)?; // Total Records column
    sheet.set_column_width(7, 15)?; // Reviewed Count column
    sheet.set_column_width(8, 15)?; // Completion % column

    let mut current_row = 0u32;

    // Calculate overall statistics
    let mut total_entity_pending = 0i64;
    let mut total_entity_reviewed = 0i64;
    let mut total_service_pending = 0i64;
    let mut total_service_reviewed = 0i64;

    for user in &data {
        total_entity_pending += user.entity_stats.pending_review;
        total_entity_reviewed += user.entity_stats.reviewed_count;
        total_service_pending += user.service_stats.pending_review;
        total_service_reviewed += user.service_stats.reviewed_count;
    }

    let total_pending = total_entity_pending + total_service_pending;
    let total_reviewed = total_entity_reviewed + total_service_reviewed;
    let total_all = total_pending + total_reviewed;
    let overall_percentage = if total_all > 0 {
        (total_reviewed as f64 / total_all as f64) * 100.0
    } else {
        0.0
    };

    // Create format for percentages
    let percentage_format = Format::new().set_num_format("0.0");

    // Write overall summary section
    sheet.write_string(current_row, 0, "OVERALL PROGRESS SUMMARY")?;
    current_row += 1;
    sheet.write_string(current_row, 0, "")?; // Empty row for spacing
    current_row += 1;

    // Overall stats headers
    let summary_headers = vec![
        "Metric", "Entity Records", "Service Records", "Total Records"
    ];
    for (col_num, header) in summary_headers.iter().enumerate() {
        sheet.write_string(current_row, col_num as u16, *header)?;
    }
    current_row += 1;

    // Overall stats data
    let summary_rows = vec![
        ("Pending Review", total_entity_pending, total_service_pending, total_pending),
        ("Reviewed (Confirmed)", total_entity_reviewed, total_service_reviewed, total_reviewed),
        ("Total Records", total_entity_pending + total_entity_reviewed, total_service_pending + total_service_reviewed, total_all),
    ];

    for (metric, entity_count, service_count, total_count) in summary_rows {
        sheet.write_string(current_row, 0, metric)?;
        sheet.write_number(current_row, 1, entity_count as f64)?;
        sheet.write_number(current_row, 2, service_count as f64)?;
        sheet.write_number(current_row, 3, total_count as f64)?;
        current_row += 1;
    }

    // Overall completion percentage
    sheet.write_string(current_row, 0, "Overall Completion %")?;
    sheet.write_string(current_row, 1, "")?;
    sheet.write_string(current_row, 2, "")?;
    sheet.write_number_with_format(current_row, 3, overall_percentage, &percentage_format)?;
    current_row += 2; // Extra spacing

    // Write detailed user breakdown section
    sheet.write_string(current_row, 0, "USER BREAKDOWN")?;
    current_row += 1;
    sheet.write_string(current_row, 0, "")?; // Empty row for spacing
    current_row += 1;

    // User breakdown headers
    let user_headers = vec![
        "User", "User Prefix", "Record Type", "Pending Review", "Confirmed Match", 
        "Confirmed Non-Match", "Total Records", "Reviewed Count", "Completion %"
    ];
    for (col_num, header) in user_headers.iter().enumerate() {
        sheet.write_string(current_row, col_num as u16, *header)?;
    }
    current_row += 1;

    // User breakdown data
    for user in &data {
        // Entity row
        sheet.write_string(current_row, 0, &user.username)?;
        sheet.write_string(current_row, 1, &user.user_prefix)?;
        sheet.write_string(current_row, 2, "Entity")?;
        sheet.write_number(current_row, 3, user.entity_stats.pending_review as f64)?;
        sheet.write_number(current_row, 4, user.entity_stats.confirmed_match as f64)?;
        sheet.write_number(current_row, 5, user.entity_stats.confirmed_non_match as f64)?;
        sheet.write_number(current_row, 6, user.entity_stats.total as f64)?;
        sheet.write_number(current_row, 7, user.entity_stats.reviewed_count as f64)?;
        sheet.write_number_with_format(current_row, 8, user.entity_stats.review_percentage, &percentage_format)?;
        current_row += 1;

        // Service row
        sheet.write_string(current_row, 0, &user.username)?;
        sheet.write_string(current_row, 1, &user.user_prefix)?;
        sheet.write_string(current_row, 2, "Service")?;
        sheet.write_number(current_row, 3, user.service_stats.pending_review as f64)?;
        sheet.write_number(current_row, 4, user.service_stats.confirmed_match as f64)?;
        sheet.write_number(current_row, 5, user.service_stats.confirmed_non_match as f64)?;
        sheet.write_number(current_row, 6, user.service_stats.total as f64)?;
        sheet.write_number(current_row, 7, user.service_stats.reviewed_count as f64)?;
        sheet.write_number_with_format(current_row, 8, user.service_stats.review_percentage, &percentage_format)?;
        current_row += 1;

        // Add a blank row between users for readability
        sheet.write_string(current_row, 0, "")?;
        current_row += 1;
    }

    // Add timestamp
    current_row += 1;
    sheet.write_string(current_row, 0, "Generated")?;
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sheet.write_string(current_row, 1, &timestamp)?;

    info!("'Progress Overview' sheet written with data for {} users.", data.len());
    Ok(())
}