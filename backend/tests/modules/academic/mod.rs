use chrono::NaiveDate;
use eravaya::modules::academic::calendar::{
    CalendarHolidayCheckResult, CreateCalendarEntryRequest,
};
use eravaya::modules::academic::handler::SubjectTypeInput;
use uuid::Uuid;

#[test]
fn test_create_calendar_entry_request_serialization() {
    let req = CreateCalendarEntryRequest {
        date: NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(),
        end_date: None,
        entry_type: Some("HOLIDAY".into()),
        title: "Christmas Day".into(),
        description: Some("Winter vacation day".into()),
        is_working_day: Some(false),
        applies_to_all_classes: Some(true),
        class_ids: None,
    };

    let serialized = serde_json::to_string(&req).unwrap();
    assert!(serialized.contains("Christmas Day"));
    assert!(serialized.contains("2026-12-25"));
    assert!(serialized.contains("HOLIDAY"));
}

#[test]
fn test_calendar_holiday_check_result_model() {
    let class_id = Uuid::new_v4();
    let result = CalendarHolidayCheckResult {
        date: NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
        is_holiday: true,
        is_working_day: false,
        title: Some("Independence Day".into()),
        description: Some("National Holiday".into()),
        entry_type: Some("HOLIDAY".into()),
        applies_to_all_classes: false,
        affected_class_ids: vec![class_id],
        affected_class_names: vec!["Class 10".into()],
    };

    assert!(result.is_holiday);
    assert!(!result.is_working_day);
    assert_eq!(result.affected_class_ids.len(), 1);
    assert_eq!(result.affected_class_names[0], "Class 10");
}

#[test]
fn test_subject_type_input_payload() {
    let input = SubjectTypeInput {
        name: "STEM Elective".into(),
        code: "STEM".into(),
        description: Some("Science, Tech, Engineering & Math".into()),
        system_subject_type: None,
        is_elective: Some(true),
        is_mandatory: Some(false),
    };

    assert_eq!(input.name, "STEM Elective");
    assert_eq!(input.is_elective, Some(true));
    assert_eq!(input.is_mandatory, Some(false));
}
