use chrono::NaiveDate;
use eravaya::modules::whatsapp::menus::attendance::{
    extract_rolls_from_slice, parse_attendance_command, parse_date_input, AttendanceInputParsed,
};

#[test]
fn test_attendance_all_present_keywords() {
    assert_eq!(
        parse_attendance_command("all present").unwrap(),
        AttendanceInputParsed::AllPresent
    );
    assert_eq!(
        parse_attendance_command("everyone present").unwrap(),
        AttendanceInputParsed::AllPresent
    );
    assert_eq!(
        parse_attendance_command("0 absent").unwrap(),
        AttendanceInputParsed::AllPresent
    );
    assert_eq!(
        parse_attendance_command("nil").unwrap(),
        AttendanceInputParsed::AllPresent
    );
}

#[test]
fn test_attendance_directives_absent_list() {
    let parsed = parse_attendance_command("1, 5, 23 absent").unwrap();
    match parsed {
        AttendanceInputParsed::Directives {
            absent_rolls,
            present_rolls,
            is_replace,
            is_present_only,
        } => {
            assert_eq!(absent_rolls, vec!["1", "5", "23"]);
            assert!(present_rolls.is_empty());
            assert!(!is_replace);
            assert!(!is_present_only);
        }
        other => panic!("Expected directives, got: {:?}", other),
    }
}

#[test]
fn test_attendance_directives_present_only() {
    let parsed = parse_attendance_command("4 present").unwrap();
    match parsed {
        AttendanceInputParsed::Directives {
            present_rolls,
            absent_rolls,
            is_present_only,
            ..
        } => {
            assert_eq!(present_rolls, vec!["4"]);
            assert!(absent_rolls.is_empty());
            assert!(is_present_only);
        }
        other => panic!("Expected present only, got: {:?}", other),
    }
}

#[test]
fn test_extract_rolls_from_slice() {
    let raw = "Roll no 01, 005, 12 and 12 are absent";
    let rolls = extract_rolls_from_slice(raw);
    assert_eq!(rolls, vec!["1", "5", "12"]);
}

#[test]
fn test_parse_date_input_relative_dates() {
    let base_date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();

    let today = parse_date_input("today", base_date).unwrap().unwrap();
    assert_eq!(today, base_date);

    let yesterday = parse_date_input("yesterday", base_date).unwrap().unwrap();
    assert_eq!(yesterday, NaiveDate::from_ymd_opt(2026, 11, 8).unwrap());

    let day_before = parse_date_input("day before yesterday", base_date)
        .unwrap()
        .unwrap();
    assert_eq!(day_before, NaiveDate::from_ymd_opt(2026, 11, 7).unwrap());

    let explicit = parse_date_input("2026-11-05", base_date).unwrap().unwrap();
    assert_eq!(explicit, NaiveDate::from_ymd_opt(2026, 11, 5).unwrap());
}
