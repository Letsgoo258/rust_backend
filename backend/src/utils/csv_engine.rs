use std::collections::HashMap;

#[derive(Clone)]
#[allow(dead_code)]
pub enum ColumnType {
    String,
    Email,
    Phone,
    Date,
    Int,
}

#[derive(Clone)]
pub struct ColumnSchema {
    pub header: &'static str,
    pub col_type: ColumnType,
    pub required: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

pub struct CsvSchema {
    pub columns: Vec<ColumnSchema>,
}

#[allow(dead_code)]
pub struct CsvParseResult {
    pub valid_rows: Vec<HashMap<String, String>>,
    pub failed_csv: Option<String>, // Base64 encoded CSV string
    pub success_count: usize,
    pub failed_count: usize,
}

impl CsvSchema {
    pub fn parse(&self, raw_data: &[u8]) -> Result<CsvParseResult, String> {
        let mut rdr = csv::ReaderBuilder::new().from_reader(raw_data);
        let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
        
        let mut valid_rows = Vec::new();
        let mut failed_records = Vec::new();
        
        // Find indices
        let mut col_indices: HashMap<&str, usize> = HashMap::new();
        for (i, h) in headers.iter().enumerate() {
            col_indices.insert(h.trim(), i);
        }
        
        for col in &self.columns {
            if col.required && !col_indices.contains_key(col.header) {
                return Err(format!("Missing required column: {}", col.header));
            }
        }
        
        for result in rdr.records() {
            let record = match result {
                Ok(r) => r,
                Err(_) => continue,
            };
            
            let mut row_map = HashMap::new();
            let mut error_msg = None;
            
            for col in &self.columns {
                let val = col_indices.get(col.header).and_then(|&idx| record.get(idx)).unwrap_or("").trim();
                
                if col.required && val.is_empty() {
                    error_msg = Some(format!("{} is required", col.header));
                    break;
                }
                
                if !val.is_empty() {
                    if let Some(min) = col.min_length {
                        if val.len() < min {
                            error_msg = Some(format!("{} is too short (min {})", col.header, min));
                            break;
                        }
                    }
                    if let Some(max) = col.max_length {
                        if val.len() > max {
                            error_msg = Some(format!("{} is too long (max {})", col.header, max));
                            break;
                        }
                    }
                    
                    match col.col_type {
                        ColumnType::Email => {
                            if !val.contains('@') {
                                error_msg = Some(format!("{} is invalid email", col.header));
                                break;
                            }
                        }
                        ColumnType::Phone => {
                            if val.len() < 10 || !val.chars().all(|c| c.is_digit(10) || c == '+') {
                                error_msg = Some(format!("{} is invalid phone", col.header));
                                break;
                            }
                        }
                        ColumnType::Int => {
                            if val.parse::<i64>().is_err() {
                                error_msg = Some(format!("{} must be integer", col.header));
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                
                row_map.insert(col.header.to_string(), val.to_string());
            }
            
            if let Some(err) = error_msg {
                let mut failed_record: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                failed_record.push(err);
                failed_records.push(failed_record);
            } else {
                valid_rows.push(row_map);
            }
        }
        
        let success_count = valid_rows.len();
        let failed_count = failed_records.len();
        let mut failed_csv = None;
        
        if failed_count > 0 {
            let mut wtr = csv::Writer::from_writer(vec![]);
            
            let mut err_headers: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            err_headers.push("Error_Reason".to_string());
            let _ = wtr.write_record(&err_headers);
            
            for fr in failed_records {
                let _ = wtr.write_record(&fr);
            }
            
            if let Ok(bytes) = wtr.into_inner() {
                use base64::{Engine as _, engine::general_purpose::STANDARD};
                failed_csv = Some(STANDARD.encode(&bytes));
            }
        }
        
        Ok(CsvParseResult {
            valid_rows,
            failed_csv,
            success_count,
            failed_count,
        })
    }
}
