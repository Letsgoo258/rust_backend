use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Filter input types supported across the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    Text,
    DateRange,
    Select,
    MultiSelect,
    StatusToggle,
    NumberRange,
}

/// Configuration for an individual filter field in a page context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterFieldConfig {
    pub key: String,
    pub label: String,
    pub field_type: FilterType,
    pub placeholder: Option<String>,
    pub options: Option<Vec<FilterOption>>,
    #[serde(rename = "helpLink", skip_serializing_if = "Option::is_none")]
    pub help_link: Option<String>,
    #[serde(skip_deserializing, default)]
    pub db_column: String,
    #[serde(skip_deserializing, default)]
    pub is_having_clause: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterOption {
    pub label: String,
    pub value: String,
}

/// Page / Entity filter schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageFilterSchema {
    pub context: String,
    pub title: String,
    pub fields: Vec<FilterFieldConfig>,
}

pub struct FilterEngine;

impl FilterEngine {
    /// Retrieve the filter schema for a given page/resource context
    pub fn get_schema(context: &str) -> Option<PageFilterSchema> {
        match context {
            "users" => Some(PageFilterSchema {
                context: "users".to_string(),
                title: "Users Directory Filters".to_string(),
                fields: vec![
                    FilterFieldConfig {
                        key: "role".to_string(),
                        label: "User Role".to_string(),
                        field_type: FilterType::Select,
                        placeholder: Some("All Roles".to_string()),
                        options: Some(vec![
                            FilterOption {
                                label: "All Roles".to_string(),
                                value: "ALL".to_string(),
                            },
                            FilterOption {
                                label: "Students".to_string(),
                                value: "STUDENT".to_string(),
                            },
                            FilterOption {
                                label: "Staff & Teachers".to_string(),
                                value: "TEACHER".to_string(),
                            },
                            FilterOption {
                                label: "Admins".to_string(),
                                value: "SCHOOL_ADMIN".to_string(),
                            },
                        ]),
                        db_column: "r.code".to_string(),
                        is_having_clause: true,
                        help_link: None,
                    },
                    FilterFieldConfig {
                        key: "admission_number".to_string(),
                        label: "Admission Number".to_string(),
                        field_type: FilterType::Text,
                        placeholder: Some("e.g. ADM20260100".to_string()),
                        options: None,

                        db_column: "s.admission_number".to_string(),
                        is_having_clause: false,
                        help_link: None,
                    },
                    FilterFieldConfig {
                        key: "admitted_from".to_string(),
                        label: "Admitted From".to_string(),
                        field_type: FilterType::DateRange,
                        placeholder: Some("YYYY-MM-DD".to_string()),
                        options: None,

                        db_column: "u.created_at".to_string(),
                        is_having_clause: false,
                        help_link: None,
                    },
                    FilterFieldConfig {
                        key: "admitted_to".to_string(),
                        label: "Admitted To".to_string(),
                        field_type: FilterType::DateRange,
                        placeholder: Some("YYYY-MM-DD".to_string()),
                        options: None,

                        db_column: "u.created_at".to_string(),
                        is_having_clause: false,
                        help_link: None,
                    },
                    FilterFieldConfig {
                        key: "class_name".to_string(),
                        label: "Class / Grade".to_string(),
                        field_type: FilterType::Text,
                        placeholder: Some("e.g. Grade 10 or Class 1".to_string()),
                        options: None,

                        db_column: "c.name".to_string(),
                        is_having_clause: false,
                        help_link: None,
                    },
                    FilterFieldConfig {
                        key: "status".to_string(),
                        label: "Status".to_string(),
                        field_type: FilterType::StatusToggle,
                        placeholder: None,
                        options: Some(vec![
                            FilterOption {
                                label: "All".to_string(),
                                value: "ALL".to_string(),
                            },
                            FilterOption {
                                label: "Provisional".to_string(),
                                value: "PROVISIONAL".to_string(),
                            },
                            FilterOption {
                                label: "Active".to_string(),
                                value: "ACTIVE".to_string(),
                            },
                            FilterOption {
                                label: "Graduated".to_string(),
                                value: "GRADUATED".to_string(),
                            },
                            FilterOption {
                                label: "Alumni".to_string(),
                                value: "ALUMNI".to_string(),
                            },
                            FilterOption {
                                label: "Inactive".to_string(),
                                value: "INACTIVE".to_string(),
                            },
                            FilterOption {
                                label: "Suspended".to_string(),
                                value: "SUSPENDED".to_string(),
                            },
                            FilterOption {
                                label: "Expelled".to_string(),
                                value: "EXPELLED".to_string(),
                            },
                            FilterOption {
                                label: "On Leave".to_string(),
                                value: "ON_LEAVE".to_string(),
                            },
                            FilterOption {
                                label: "Dropout".to_string(),
                                value: "DROPOUT".to_string(),
                            },
                            FilterOption {
                                label: "Readmitted".to_string(),
                                value: "READMITTED".to_string(),
                            },
                            FilterOption {
                                label: "Detained".to_string(),
                                value: "DETAINED".to_string(),
                            },
                            FilterOption {
                                label: "TC in Process".to_string(),
                                value: "TC_PROCESS".to_string(),
                            },
                            FilterOption {
                                label: "TC Issued".to_string(),
                                value: "TC_ISSUED".to_string(),
                            },
                            FilterOption {
                                label: "Transferred".to_string(),
                                value: "TRANSFERRED".to_string(),
                            },
                            FilterOption {
                                label: "TC Cancelled".to_string(),
                                value: "TC_CANCELLED".to_string(),
                            },
                        ]),
                        db_column: "s.status".to_string(),
                        is_having_clause: false,
                        help_link: Some("/dashboard/users/status-guide".to_string()),
                    },
                ],
            }),
            _ => None,
        }
    }

    /// Intelligently sanitize input parameters:
    /// 1. Drops any parameter not configured for this context.
    /// 2. Validates and normalizes non-empty values.
    /// 3. Returns the clean sanitized params map.
    pub fn sanitize_params(
        context: &str,
        raw_params: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let schema = match Self::get_schema(context) {
            Some(s) => s,
            None => return HashMap::new(),
        };

        let mut sanitized = HashMap::new();
        let allowed_keys: std::collections::HashSet<&str> =
            schema.fields.iter().map(|f| f.key.as_str()).collect();

        for (key, val) in raw_params {
            let trimmed = val.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
                continue;
            }

            if allowed_keys.contains(key.as_str()) || key == "search" {
                sanitized.insert(key.clone(), trimmed.to_string());
            }
        }

        sanitized
    }
}
