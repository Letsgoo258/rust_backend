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

        // Helper to normalize header keys: lowercased, alphanumeric only
        fn normalize_key(s: &str) -> String {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        }

        // Map normalized header strings to their column index
        let mut col_indices: HashMap<String, usize> = HashMap::new();
        for (i, h) in headers.iter().enumerate() {
            let trimmed = h.trim();
            // Store raw trimmed
            col_indices.insert(trimmed.to_string(), i);
            // Store normalized alphanumeric
            col_indices.insert(normalize_key(trimmed), i);
        }

        // Helper to find column index from schema header or standard variations
        let find_col_idx = |col_header: &str| -> Option<usize> {
            if let Some(&idx) = col_indices.get(col_header) {
                return Some(idx);
            }
            let norm = normalize_key(col_header);
            if let Some(&idx) = col_indices.get(&norm) {
                return Some(idx);
            }
            // Additional aliases
            match norm.as_str() {
                "dateofbirth" | "dob" | "birthdate" => {
                    for candidate in ["dateofbirth", "dob", "birthdate", "dateofbirthyyyymmdd"] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "admissionnumber" | "admissionno" | "admno" | "admnumber" => {
                    for candidate in [
                        "admissionnumber",
                        "admissionno",
                        "admno",
                        "admnumber",
                        "regnumber",
                        "regno",
                    ] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "firstname" | "fname" => {
                    for candidate in ["firstname", "fname", "first"] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "lastname" | "lname" | "surname" => {
                    for candidate in ["lastname", "lname", "surname", "last"] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "guardianphone" | "parentphone" => {
                    for candidate in [
                        "guardianphone",
                        "parentphone",
                        "fatherphone",
                        "motherphone",
                        "gphone",
                        "guardianmobile",
                    ] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "guardianemail" | "parentemail" => {
                    for candidate in [
                        "guardianemail",
                        "parentemail",
                        "fatheremail",
                        "motheremail",
                        "gemail",
                    ] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "guardianfirstname" | "parentfirstname" => {
                    for candidate in [
                        "guardianfirstname",
                        "parentfirstname",
                        "guardianname",
                        "parentname",
                        "gfirstname",
                    ] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                "guardianlastname" | "parentlastname" => {
                    for candidate in ["guardianlastname", "parentlastname", "glastname"] {
                        if let Some(&idx) = col_indices.get(candidate) {
                            return Some(idx);
                        }
                    }
                }
                _ => {}
            }
            None
        };

        for col in &self.columns {
            if col.required && find_col_idx(col.header).is_none() {
                return Err(format!("Missing required column: {}", col.header));
            }
        }

        let mut valid_rows = Vec::new();
        let mut failed_records = Vec::new();

        for result in rdr.records() {
            let record = match result {
                Ok(r) => r,
                Err(_) => continue,
            };

            let mut row_map = HashMap::new();
            let mut error_msg = None;

            for col in &self.columns {
                let val = find_col_idx(col.header)
                    .and_then(|idx| record.get(idx))
                    .unwrap_or("")
                    .trim();

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
                            let digit_count = val.chars().filter(|c| c.is_ascii_digit()).count();
                            if digit_count < 10 {
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
                use base64::{engine::general_purpose::STANDARD, Engine as _};
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
