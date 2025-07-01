use anyhow::Result;
use rust_xlsxwriter::{Workbook, FormatAlign, Worksheet};
use std::path::Path;
use log::info;

use crate::models::{OrganizationExportRow, ServiceExportRow};

/// Writes the extracted organization and service data to an Excel file with two sheets.
pub async fn write_excel_file(
    file_path: &Path,
    org_data: Vec<OrganizationExportRow>,
    svc_data: Vec<ServiceExportRow>,
) -> Result<()> {
    info!("Initializing Excel workbook for file: {:?}", file_path);
    let mut workbook = Workbook::new();

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