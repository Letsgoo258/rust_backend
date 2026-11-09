use eravaya::modules::users::profile::BulkUpdateUsersRequest;
use eravaya::utils::student_lifecycle::StudentLifecycleStatus;

#[test]
fn test_student_lifecycle_complete_transition_rules() {
    let provisional = StudentLifecycleStatus::Provisional;
    let active = StudentLifecycleStatus::Active;
    let graduated = StudentLifecycleStatus::Graduated;
    let alumni = StudentLifecycleStatus::Alumni;
    let suspended = StudentLifecycleStatus::Suspended;
    let expelled = StudentLifecycleStatus::Expelled;
    let tc_process = StudentLifecycleStatus::TcProcess;
    let tc_issued = StudentLifecycleStatus::TcIssued;
    let transferred = StudentLifecycleStatus::Transferred;

    // Provisional -> Active
    assert!(provisional.can_transition_to(active));

    // Active -> Graduated -> Alumni
    assert!(active.can_transition_to(graduated));
    assert!(graduated.can_transition_to(alumni));

    // Terminal states cannot transition to anything
    assert!(alumni.valid_transitions().is_empty());
    assert!(expelled.valid_transitions().is_empty());
    assert!(transferred.valid_transitions().is_empty());

    // Suspended cannot directly transition to Graduated
    assert!(!suspended.can_transition_to(graduated));

    // TC flow
    assert!(active.can_transition_to(tc_process));
    assert!(tc_process.can_transition_to(tc_issued));
    assert!(tc_issued.can_transition_to(transferred));
    assert!(!tc_process.can_transition_to(alumni));
}

#[test]
fn test_student_lifecycle_string_representations() {
    assert_eq!(StudentLifecycleStatus::Active.as_str(), "ACTIVE");
    assert_eq!(StudentLifecycleStatus::TcIssued.as_str(), "TC_ISSUED");
    assert_eq!(StudentLifecycleStatus::TcProcess.as_str(), "TC_PROCESS");
    assert_eq!(StudentLifecycleStatus::TcCancelled.as_str(), "TC_CANCELLED");

    assert_eq!(StudentLifecycleStatus::Active.display_label(), "Active");
    assert_eq!(
        StudentLifecycleStatus::TcIssued.display_label(),
        "TC Issued"
    );
    assert_eq!(
        StudentLifecycleStatus::TcProcess.display_label(),
        "TC in Process"
    );
}

#[test]
fn test_bulk_update_users_request_deserialization() {
    let json_str = r#"{
        "users": [
            {
                "id": "11111111-2222-3333-4444-555555555555",
                "first_name": "Aarav",
                "last_name": "Patel",
                "admission_number": "ADM-2026-001",
                "status": "ACTIVE"
            }
        ]
    }"#;

    let payload: BulkUpdateUsersRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(payload.users.len(), 1);
    let user = &payload.users[0];
    assert_eq!(user.first_name.as_deref(), Some("Aarav"));
    assert_eq!(user.admission_number.as_deref(), Some("ADM-2026-001"));
    assert_eq!(user.status.as_deref(), Some("ACTIVE"));
}
