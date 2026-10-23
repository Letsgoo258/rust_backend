use super::catalogue::catalogue;
use std::collections::HashSet;

/// Returns the baseline enabled capabilities for all 29 Indian school system roles.
pub fn default_enabled_keys(role_code: &str) -> HashSet<&'static str> {
    let code = role_code.to_uppercase();
    let mut set = HashSet::new();

    match code.as_str() {
        // 1. Leadership & Governance (Full Platform / School Authority)
        "SUPER_ADMIN" | "SCHOOL_ADMIN" | "HEADMASTER" | "PRINCIPAL" => {
            for (key, _, _, _) in catalogue() {
                set.insert(key);
            }
        }

        // 2. Deputy Head
        "VICE_PRINCIPAL" => {
            set.insert("users:view_directory");
            set.insert("users:export");
            set.insert("certificate:issue_tc");
            set.insert("certificate:manage_types");
            set.insert("academics:view");
            set.insert("academics:manage_curriculum");
            set.insert("academics:assign_students");
            set.insert("academics:manage_timetable");
            set.insert("academics:assign_class_teacher");
            set.insert("academics:edit_class_teacher");
            set.insert("academics:delete_class_teacher");
            set.insert("exams:manage_schedules");
            set.insert("exams:enter_marks");
            set.insert("settings:view");
        }

        // 3. Subject Coordinators & Department Heads
        "SUBJECT_COORDINATOR" => {
            set.insert("users:view_directory");
            set.insert("academics:view");
            set.insert("academics:manage_curriculum");
            set.insert("exams:manage_schedules");
            set.insert("exams:enter_marks");
        }

        // 4. Examination Coordinator
        "EXAM_COORDINATOR" => {
            set.insert("users:view_directory");
            set.insert("academics:view");
            set.insert("exams:manage_schedules");
            set.insert("exams:enter_marks");
            set.insert("users:export");
        }

        // 5. Class Teachers
        "CLASS_TEACHER" => {
            set.insert("users:view_directory");
            set.insert("users:export");
            set.insert("certificate:issue_tc");
            set.insert("academics:view");
            set.insert("academics:assign_students");
            set.insert("exams:enter_marks");
        }

        // 6. Regular Teachers & Faculty
        "TEACHER" | "PT_TEACHER" | "PT TEACHER" => {
            set.insert("users:view_directory");
            set.insert("users:export");
            set.insert("certificate:issue_tc");
            set.insert("academics:view");
            set.insert("exams:enter_marks");
        }

        // 7. Academic Support: Lab Assistant & Counselor
        "LAB_ASSISTANT" | "LAB ASSISTANT" => {
            set.insert("users:view_directory");
            set.insert("academics:view");
        }

        "COUNSELOR" => {
            set.insert("users:view_directory");
            set.insert("academics:view");
        }

        // 8. Administrative Staff, Clerks, Receptionists
        "STAFF" | "CLERK" | "RECEPTIONIST" => {
            set.insert("users:view_directory");
            set.insert("users:admission_register");
            set.insert("bulk:edit_multiple_fields");
            set.insert("users:export");
            set.insert("certificate:issue_tc");
            // users:bulk_onboard is STRICTLY restricted to School Admin
        }

        // 9. Finance & Accounts
        "ACCOUNTANT" => {
            set.insert("users:view_directory");
            set.insert("users:export");
            set.insert("finance:view_fees");
            set.insert("finance:collect_fees");
        }

        // 10. Human Resources Manager
        "HR_MANAGER" | "HR MANAGER" => {
            set.insert("users:view_directory");
            set.insert("users:export");
        }

        // 11. Library Supervisor
        "LIBRARIAN" => {
            set.insert("users:view_directory");
            set.insert("library:manage_catalog");
        }

        // 12. Transport Fleet: Manager, Drivers, Attendants
        "TRANSPORT_MANAGER" | "TRANSPORT MANAGER" => {
            set.insert("users:view_directory");
            set.insert("transport:manage_routes");
            set.insert("transport:view_roster");
        }

        "DRIVER" | "BUS_ATTENDANT" | "BUS ATTENDANT" => {
            set.insert("transport:view_roster");
        }

        // 13. Cafeteria & Canteen
        "CANTEEN_MANAGER" | "CANTEEN MANAGER" => {
            set.insert("users:view_directory");
        }

        "CANTEEN_STAFF" | "CANTEEN STAFF" => {
            // Limited operations
        }

        // 14. Campus Security
        "SECURITY_HEAD" | "SECURITY HEAD" => {
            set.insert("users:view_directory");
            set.insert("transport:view_roster");
        }

        "SECURITY_GUARD" | "SECURITY GUARD" => {
            set.insert("transport:view_roster");
        }

        // 15. Operational Support: Peon, Housekeeping, General Support
        "PEON" | "HOUSEKEEPING" | "SUPPORT_STAFF" | "SUPPORT STAFF" => {
            // General support
        }

        // 16. Stakeholders: Students, Parents, Alumni
        "STUDENT" | "PARENT" => {
            set.insert("academics:view");
            set.insert("finance:view_fees");
        }

        "ALUMNI" => {
            // View alumni community
        }

        _ => {
            // Default baseline fallback
            set.insert("academics:view");
        }
    }

    set
}
