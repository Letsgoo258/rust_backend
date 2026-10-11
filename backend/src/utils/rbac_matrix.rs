use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserManagementTask {
    ViewUserList,
    AddAdminUser,
    AddStaffUser,
    BulkCsvUpload,
    EditStandardUserData,
    EditCriticalIdentityData, // Name, DOB, Email, Phone
    ChangeStudentClassAndSection,
    LockUser,
    DeleteUser,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleTaskPermissions {
    pub role_code: String,
    pub role_name: String,
    pub allowed_tasks: Vec<UserManagementTask>,
    pub denied_tasks: Vec<UserManagementTask>,
}

#[allow(dead_code)]
pub struct RbacCapabilityMatrix;

#[allow(dead_code)]
impl RbacCapabilityMatrix {
    /// Returns the complete RBAC capability matrix mapping all system roles to allowed and denied tasks
    pub fn get_matrix() -> Vec<RoleTaskPermissions> {
        let all_tasks = vec![
            UserManagementTask::ViewUserList,
            UserManagementTask::AddAdminUser,
            UserManagementTask::AddStaffUser,
            UserManagementTask::BulkCsvUpload,
            UserManagementTask::EditStandardUserData,
            UserManagementTask::EditCriticalIdentityData,
            UserManagementTask::ChangeStudentClassAndSection,
            UserManagementTask::LockUser,
            UserManagementTask::DeleteUser,
        ];

        let role_definitions = vec![
            ("SUPER_ADMIN", "Super Admin", all_tasks.clone()),
            ("SCHOOL_ADMIN", "School Admin", all_tasks.clone()),
            (
                "VICE_PRINCIPAL",
                "Vice Principal",
                vec![
                    UserManagementTask::ViewUserList,
                    UserManagementTask::EditStandardUserData,
                    UserManagementTask::ChangeStudentClassAndSection,
                ],
            ),
            (
                "HEADMASTER",
                "Headmaster",
                vec![
                    UserManagementTask::ViewUserList,
                    UserManagementTask::EditStandardUserData,
                    UserManagementTask::ChangeStudentClassAndSection,
                ],
            ),
            (
                "HR_MANAGER",
                "HR Manager",
                vec![
                    UserManagementTask::ViewUserList,
                    UserManagementTask::AddStaffUser,
                    UserManagementTask::EditStandardUserData,
                ],
            ),
            ("TEACHER", "Teacher", vec![UserManagementTask::ViewUserList]),
            (
                "CLASS_TEACHER",
                "Class Teacher",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "SUBJECT_COORDINATOR",
                "Subject Coordinator",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "EXAM_COORDINATOR",
                "Exam Coordinator",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "LIBRARIAN",
                "Librarian",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "LAB_ASSISTANT",
                "Lab Assistant",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "COUNSELOR",
                "Counselor",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "PT_TEACHER",
                "PT Teacher",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "RECEPTIONIST",
                "Receptionist",
                vec![UserManagementTask::ViewUserList],
            ),
            (
                "ACCOUNTANT",
                "Accountant",
                vec![UserManagementTask::ViewUserList],
            ),
            ("CLERK", "Clerk", vec![UserManagementTask::ViewUserList]),
            (
                "TRANSPORT_MANAGER",
                "Transport Manager",
                vec![UserManagementTask::ViewUserList],
            ),
            ("DRIVER", "Driver", vec![]),
            ("BUS_ATTENDANT", "Bus Attendant", vec![]),
            ("CANTEEN_MANAGER", "Canteen Manager", vec![]),
            ("CANTEEN_STAFF", "Canteen Staff", vec![]),
            ("SECURITY_HEAD", "Security Head", vec![]),
            ("SECURITY_GUARD", "Security Guard", vec![]),
            ("PEON", "Peon", vec![]),
            ("HOUSEKEEPING", "Housekeeping", vec![]),
            ("SUPPORT_STAFF", "Support Staff", vec![]),
            ("ALUMNI", "Alumni", vec![]),
            ("PARENT", "Parent", vec![]),
            ("STUDENT", "Student", vec![]),
            ("NO_CAPABILITY", "No Capability User", vec![]),
        ];

        role_definitions
            .into_iter()
            .map(|(code, name, allowed)| {
                let denied = all_tasks
                    .iter()
                    .filter(|t| !allowed.contains(t))
                    .cloned()
                    .collect();

                RoleTaskPermissions {
                    role_code: code.to_string(),
                    role_name: name.to_string(),
                    allowed_tasks: allowed,
                    denied_tasks: denied,
                }
            })
            .collect()
    }

    /// Checks whether a given role code has permission to execute a specific task
    pub fn is_authorized(role_codes: &[String], task: &UserManagementTask) -> bool {
        let matrix = Self::get_matrix();
        for code in role_codes {
            let code_upper = code.trim().to_uppercase();
            if let Some(perm) = matrix.iter().find(|m| m.role_code == code_upper) {
                if perm.allowed_tasks.contains(task) {
                    return true;
                }
            }
        }
        false
    }
}
