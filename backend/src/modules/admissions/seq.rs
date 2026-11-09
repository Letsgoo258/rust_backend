use crate::{
    error::AppError,
    routes::settings::admissions::{AdmissionConfigPayload, RequiredDocItem},
};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdmissionSequencePreview {
    pub next_admission_number: String,
    pub sample_preview: String,
    pub format_pattern: String,
    pub current_count: i64,
    pub prefix: String,
    pub separator: String,
    pub start_number: String,
    pub year_in_format: bool,
    pub academic_year_name: String,
}

/// Helper to load the current admission config for a school, filling defaults if not yet configured.
pub async fn get_school_admission_config(
    conn: &mut sqlx::PgConnection,
    school_id: Uuid,
) -> Result<AdmissionConfigPayload, AppError> {
    // 1. Fetch active academic year
    let active_ay = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, name
        FROM academic_years
        WHERE school_id = $1 AND status = 'ACTIVE'
        ORDER BY start_date DESC
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .fetch_optional(&mut *conn)
    .await?;

    let current_year = Utc::now().date_naive().year();
    let default_ay_name = format!("{}-{}", current_year, current_year + 1);
    let resolved_ay_id = active_ay.as_ref().map(|a| a.0);
    let resolved_ay_name = active_ay
        .as_ref()
        .map(|a| a.1.clone())
        .unwrap_or(default_ay_name);

    // 2. Fetch saved admission settings
    let setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'admission_settings'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&mut *conn)
    .await?;

    let default_docs = vec![
        RequiredDocItem {
            id: "birth_cert".into(),
            label: "Birth Certificate".into(),
            required: true,
        },
        RequiredDocItem {
            id: "prev_report".into(),
            label: "Previous Year Report Card".into(),
            required: true,
        },
        RequiredDocItem {
            id: "photo".into(),
            label: "Passport-size Photographs (4 copies)".into(),
            required: true,
        },
    ];

    let config: AdmissionConfigPayload = if let Some(val) = setting_val {
        let mut parsed: AdmissionConfigPayload =
            serde_json::from_value(val).unwrap_or_else(|_| AdmissionConfigPayload {
                academic_year_id: resolved_ay_id,
                academic_year_name: Some(resolved_ay_name.clone()),
                prefix: "ADM".into(),
                separator: "-".into(),
                start_number: "001".into(),
                year_in_format: true,
                intake_capacity: 500,
                open_date: format!("{}-01-01", current_year),
                close_date: format!("{}-12-31", current_year),
                required_docs: default_docs,
                is_open: Some(true),
            });
        if parsed.academic_year_name.is_none() || parsed.academic_year_name.as_deref() == Some("") {
            parsed.academic_year_name = Some(resolved_ay_name);
        }
        if parsed.academic_year_id.is_none() {
            parsed.academic_year_id = resolved_ay_id;
        }
        parsed
    } else {
        AdmissionConfigPayload {
            academic_year_id: resolved_ay_id,
            academic_year_name: Some(resolved_ay_name),
            prefix: "ADM".into(),
            separator: "-".into(),
            start_number: "001".into(),
            year_in_format: true,
            intake_capacity: 500,
            open_date: format!("{}-01-01", current_year),
            close_date: format!("{}-12-31", current_year),
            required_docs: default_docs,
            is_open: Some(true),
        }
    };

    Ok(config)
}

/// Constructs the static prefix part of the admission number based on config.
pub fn build_prefix_part(config: &AdmissionConfigPayload) -> (String, String) {
    let prefix = config.prefix.trim().to_uppercase();
    let sep = &config.separator;
    let ay_name = config
        .academic_year_name
        .clone()
        .unwrap_or_else(|| "2026-2027".into());

    let (prefix_part, format_pattern) = if config.year_in_format {
        (
            format!("{}{}{}{}", prefix, sep, ay_name, sep),
            format!("{}[YEAR]{}", prefix, sep),
        )
    } else if !sep.is_empty() {
        (format!("{}{}", prefix, sep), format!("{}{}", prefix, sep))
    } else {
        (prefix.clone(), prefix.clone())
    };

    (prefix_part, format_pattern)
}

/// Computes preview information for admission number sequencing.
pub async fn get_admission_sequence_preview(
    conn: &mut sqlx::PgConnection,
    school_id: Uuid,
) -> Result<AdmissionSequencePreview, AppError> {
    let config = get_school_admission_config(conn, school_id).await?;
    let (prefix_part, format_pattern_base) = build_prefix_part(&config);

    let start_num_parsed: i64 = config
        .start_number
        .trim()
        .parse::<i64>()
        .unwrap_or(1)
        .max(1);

    let pad_width = config.start_number.trim().len().max(3);
    let hashes = "#".repeat(pad_width);
    let pattern_str = format!("{}{}", format_pattern_base, hashes);

    // Fetch existing admission numbers for this school
    let existing_numbers = sqlx::query_scalar::<_, String>(
        r#"
        SELECT admission_number
        FROM users
        WHERE school_id = $1 AND admission_number IS NOT NULL AND TRIM(admission_number) != ''
        "#,
    )
    .bind(school_id)
    .fetch_all(&mut *conn)
    .await
    .unwrap_or_default();

    let prefix_upper = prefix_part.to_uppercase();
    let mut max_seq: i64 = 0;
    let mut matching_count: i64 = 0;

    for num in &existing_numbers {
        let num_clean = num.trim();
        let num_upper = num_clean.to_uppercase();
        if num_upper.starts_with(&prefix_upper) {
            matching_count += 1;
            let suffix = &num_clean[prefix_part.len()..];
            if let Ok(val) = suffix.parse::<i64>() {
                if val > max_seq {
                    max_seq = val;
                }
            }
        }
    }

    let next_seq = std::cmp::max(start_num_parsed, max_seq + 1);
    let next_number = format!("{}{:0width$}", prefix_part, next_seq, width = pad_width);
    let sample_preview = format!(
        "{}{:0width$}",
        prefix_part,
        start_num_parsed,
        width = pad_width
    );

    Ok(AdmissionSequencePreview {
        next_admission_number: next_number,
        sample_preview,
        format_pattern: pattern_str,
        current_count: matching_count,
        prefix: config.prefix,
        separator: config.separator,
        start_number: config.start_number,
        year_in_format: config.year_in_format,
        academic_year_name: config
            .academic_year_name
            .unwrap_or_else(|| "2026-2027".into()),
    })
}

/// Generates the next sequential admission number for a school inside a database transaction.
/// Uses an advisory transaction lock to guarantee race-condition free allocation.
pub async fn generate_next_admission_number(
    conn: &mut sqlx::PgConnection,
    school_id: Uuid,
) -> Result<String, AppError> {
    // 1. Transaction-level advisory lock on this school's admission sequence namespace
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext(concat($1::text, '_admission_seq')))")
        .bind(school_id)
        .execute(&mut *conn)
        .await?;

    // 2. Load admission configuration
    let config = get_school_admission_config(conn, school_id).await?;
    let (prefix_part, _) = build_prefix_part(&config);

    let start_num_parsed: i64 = config
        .start_number
        .trim()
        .parse::<i64>()
        .unwrap_or(1)
        .max(1);
    let pad_width = config.start_number.trim().len().max(3);

    // 3. Scan existing admission numbers to find the highest allocated number matching this prefix
    let existing_numbers = sqlx::query_scalar::<_, String>(
        r#"
        SELECT admission_number
        FROM users
        WHERE school_id = $1 AND admission_number IS NOT NULL AND TRIM(admission_number) != ''
        "#,
    )
    .bind(school_id)
    .fetch_all(&mut *conn)
    .await
    .unwrap_or_default();

    let prefix_upper = prefix_part.to_uppercase();
    let mut max_seq: i64 = 0;

    let existing_set: std::collections::HashSet<String> = existing_numbers
        .iter()
        .map(|s| s.trim().to_uppercase())
        .collect();

    for num in &existing_numbers {
        let num_clean = num.trim();
        let num_upper = num_clean.to_uppercase();
        if num_upper.starts_with(&prefix_upper) {
            let suffix = &num_clean[prefix_part.len()..];
            if let Ok(val) = suffix.parse::<i64>() {
                if val > max_seq {
                    max_seq = val;
                }
            }
        }
    }

    let mut next_seq = std::cmp::max(start_num_parsed, max_seq + 1);

    // Ensure absolutely no collision in case of manual non-standard entries
    let mut candidate = format!("{}{:0width$}", prefix_part, next_seq, width = pad_width);
    while existing_set.contains(&candidate.to_uppercase()) {
        next_seq += 1;
        candidate = format!("{}{:0width$}", prefix_part, next_seq, width = pad_width);
    }

    tracing::info!(
        school_id = %school_id,
        admission_number = %candidate,
        next_seq = next_seq,
        "Allocated next sequential admission number"
    );

    Ok(candidate)
}
