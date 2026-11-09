use eravaya::modules::admissions::dto::CreateAdmissionRequest;
use eravaya::modules::admissions::seq::build_prefix_part;
use eravaya::routes::settings::admissions::AdmissionConfigPayload;
use std::collections::HashMap;

#[test]
fn test_build_prefix_with_year_and_separator() {
    let config = AdmissionConfigPayload {
        academic_year_id: None,
        academic_year_name: Some("2026-2027".into()),
        prefix: "ADM".into(),
        separator: "-".into(),
        start_number: "001".into(),
        year_in_format: true,
        intake_capacity: 500,
        open_date: "2026-01-01".into(),
        close_date: "2026-12-31".into(),
        required_docs: vec![],
        is_open: Some(true),
    };

    let (prefix_part, format_pattern) = build_prefix_part(&config);
    assert_eq!(prefix_part, "ADM-2026-2027-");
    assert_eq!(format_pattern, "ADM[YEAR]-");
}

#[test]
fn test_build_prefix_without_year() {
    let config = AdmissionConfigPayload {
        academic_year_id: None,
        academic_year_name: Some("2026-2027".into()),
        prefix: "DPS".into(),
        separator: "/".into(),
        start_number: "0001".into(),
        year_in_format: false,
        intake_capacity: 250,
        open_date: "2026-01-01".into(),
        close_date: "2026-12-31".into(),
        required_docs: vec![],
        is_open: Some(true),
    };

    let (prefix_part, format_pattern) = build_prefix_part(&config);
    assert_eq!(prefix_part, "DPS/");
    assert_eq!(format_pattern, "DPS/");
}

#[test]
fn test_admission_request_serialization_roundtrip() {
    let mut custom_fields = HashMap::new();
    custom_fields.insert("previous_school".into(), "Greenwood High".into());
    custom_fields.insert("transport_required".into(), "true".into());

    let req = CreateAdmissionRequest {
        applicant_first_name: "Rahul".into(),
        applicant_middle_name: Some("K".into()),
        applicant_last_name: "Sharma".into(),
        date_of_birth: "2015-05-12".into(),
        applicant_gender: Some("MALE".into()),
        applicant_phone: Some("+919876543210".into()),
        applicant_email: Some("rahul@example.com".into()),
        guardian_name: "Karan Sharma".into(),
        guardian_relationship: "FATHER".into(),
        guardian_phone: "+919876543211".into(),
        guardian_email: Some("karan@example.com".into()),
        guardian_occupation: Some("Engineer".into()),
        custom_fields,
    };

    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: CreateAdmissionRequest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.applicant_first_name, "Rahul");
    assert_eq!(deserialized.guardian_relationship, "FATHER");
    assert_eq!(
        deserialized.custom_fields.get("previous_school").unwrap(),
        "Greenwood High"
    );
}
