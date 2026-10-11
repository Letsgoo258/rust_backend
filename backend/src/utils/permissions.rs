#![allow(dead_code)]

//! ============================================================================
//! ERAYA RBAC & GRANULAR PERMISSIONS SYSTEM
//! ============================================================================
//!
//! This module defines granular system permissions across all functional domains
//! in the Eraya School Management Platform.
//!
//! # Architecture:
//! - **Domain Enums**: Strongly typed permission enums grouped by operational domain
//!   (Platform, Users, Academics, Reports, Observe, Admissions, Modules, School Settings).
//! - **Unified Enum (`Permission`)**: A consolidated untagged enum encompassing all
//!   domain-level permissions, serializable to SCREAMING_SNAKE_CASE strings.
//! - **`PermissionController`**: High-performance role-to-permission evaluation engine
//!   providing synchronous checks (`can`, `can_any`, `can_all`) based on the caller's roles.
//! ============================================================================

use crate::modules::auth::backend::User;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// 1. PLATFORM LEVEL PERMISSIONS (Cross-Tenant / Super-Admin)
// ============================================================================

pub mod platform {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum PlatformPermission {
        PlatformSuperAdmin,
    }

    impl PlatformPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::PlatformSuperAdmin => "PLATFORM_SUPER_ADMIN",
            }
        }
    }
}
pub use platform::PlatformPermission;

// ============================================================================
// 2. USERS & DIRECTORY PERMISSIONS
// ============================================================================

pub mod users {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum UsersPermission {
        UsersView,
        UsersViewStudents,
        UsersViewTeachers,
        UsersViewStaff,
        UsersViewAdmins,
        UsersExport,
        UsersBulkDelete,
        UsersAdmissionRegister,
        UsersTransferCertificate,
        UsersCertificateTypes,
        UsersCreateAdmin,
        UsersBulkUpload,
    }

    impl UsersPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::UsersView => "USERS_VIEW",
                Self::UsersViewStudents => "USERS_VIEW_STUDENTS",
                Self::UsersViewTeachers => "USERS_VIEW_TEACHERS",
                Self::UsersViewStaff => "USERS_VIEW_STAFF",
                Self::UsersViewAdmins => "USERS_VIEW_ADMINS",
                Self::UsersExport => "USERS_EXPORT",
                Self::UsersBulkDelete => "USERS_BULK_DELETE",
                Self::UsersAdmissionRegister => "USERS_ADMISSION_REGISTER",
                Self::UsersTransferCertificate => "USERS_TRANSFER_CERTIFICATE",
                Self::UsersCertificateTypes => "USERS_CERTIFICATE_TYPES",
                Self::UsersCreateAdmin => "USERS_CREATE_ADMIN",
                Self::UsersBulkUpload => "USERS_BULK_UPLOAD",
            }
        }
    }
}
pub use users::UsersPermission;

// ============================================================================
// 3. ACADEMICS & CURRICULUM PERMISSIONS
// ============================================================================

pub mod academics {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum AcademicsPermission {
        AcademicsView,
        AcademicsManageYears,
        AcademicsManageClasses,
        AcademicsManageSubjects,
        AcademicsAssignStudents,
    }

    impl AcademicsPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::AcademicsView => "ACADEMICS_VIEW",
                Self::AcademicsManageYears => "ACADEMICS_MANAGE_YEARS",
                Self::AcademicsManageClasses => "ACADEMICS_MANAGE_CLASSES",
                Self::AcademicsManageSubjects => "ACADEMICS_MANAGE_SUBJECTS",
                Self::AcademicsAssignStudents => "ACADEMICS_ASSIGN_STUDENTS",
            }
        }
    }
}
pub use academics::AcademicsPermission;

// ============================================================================
// 4. REPORTS & ANALYTICS PERMISSIONS
// ============================================================================

pub mod reports {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum ReportsPermission {
        ReportsView,
    }

    impl ReportsPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::ReportsView => "REPORTS_VIEW",
            }
        }
    }
}
pub use reports::ReportsPermission;

// ============================================================================
// 5. OBSERVE & CLASSROOM MONITORING PERMISSIONS
// ============================================================================

pub mod observe {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum ObservePermission {
        ObserveView,
    }

    impl ObservePermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::ObserveView => "OBSERVE_VIEW",
            }
        }
    }
}
pub use observe::ObservePermission;

// ============================================================================
// 6. ADMISSIONS & ENROLLMENT PERMISSIONS
// ============================================================================

pub mod admissions {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum AdmissionsPermission {
        AdmissionsManage,
    }

    impl AdmissionsPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::AdmissionsManage => "ADMISSIONS_MANAGE",
            }
        }
    }
}
pub use admissions::AdmissionsPermission;

// ============================================================================
// 7. SYSTEM MODULES & ADDONS PERMISSIONS
// ============================================================================

pub mod modules_perm {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum ModulesPermission {
        ModulesAccess,
    }

    impl ModulesPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::ModulesAccess => "MODULES_ACCESS",
            }
        }
    }
}
pub use modules_perm::ModulesPermission;

// ============================================================================
// 8. SCHOOL SETTINGS & ADMINISTRATION PERMISSIONS
// ============================================================================

pub mod school_settings {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum SchoolSettingsPermission {
        SchoolSettingsView,
        SchoolSettingsConfig,
        SchoolSettingsPermissions,
        SchoolSettingsBackup,
        SchoolSettingsReports,
        SchoolSettingsGraphics,
    }

    impl SchoolSettingsPermission {
        pub fn as_str(&self) -> &'static str {
            match self {
                Self::SchoolSettingsView => "SCHOOL_SETTINGS_VIEW",
                Self::SchoolSettingsConfig => "SCHOOL_SETTINGS_CONFIG",
                Self::SchoolSettingsPermissions => "SCHOOL_SETTINGS_PERMISSIONS",
                Self::SchoolSettingsBackup => "SCHOOL_SETTINGS_BACKUP",
                Self::SchoolSettingsReports => "SCHOOL_SETTINGS_REPORTS",
                Self::SchoolSettingsGraphics => "SCHOOL_SETTINGS_GRAPHICS",
            }
        }
    }
}
pub use school_settings::SchoolSettingsPermission;

// ============================================================================
// UNIFIED PERMISSION ENUM
// ============================================================================

/// Consolidated permission enum representing any valid permission across all domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Permission {
    Platform(PlatformPermission),
    Users(UsersPermission),
    Academics(AcademicsPermission),
    Reports(ReportsPermission),
    Observe(ObservePermission),
    Admissions(AdmissionsPermission),
    Modules(ModulesPermission),
    SchoolSettings(SchoolSettingsPermission),
}

impl Permission {
    /// Returns the standard SCREAMING_SNAKE_CASE string representation of this permission.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Platform(p) => p.as_str(),
            Self::Users(p) => p.as_str(),
            Self::Academics(p) => p.as_str(),
            Self::Reports(p) => p.as_str(),
            Self::Observe(p) => p.as_str(),
            Self::Admissions(p) => p.as_str(),
            Self::Modules(p) => p.as_str(),
            Self::SchoolSettings(p) => p.as_str(),
        }
    }
}

// ============================================================================
// PERMISSION CONTROLLER & ACCESS EVALUATION
// ============================================================================

pub struct PermissionController;

impl PermissionController {
    /// Resolves all granted permissions for a given user based on their assigned roles.
    pub fn get_user_permissions(user: &User) -> HashSet<Permission> {
        let mut permissions = HashSet::new();
        let roles: HashSet<String> = user.roles.iter().map(|r| r.to_uppercase()).collect();

        // --------------------------------------------------------------------
        // 1. Platform Super Admin (Global Infrastructure Access)
        // --------------------------------------------------------------------
        if roles.contains("SUPER_ADMIN") {
            permissions.insert(Permission::Platform(PlatformPermission::PlatformSuperAdmin));
            permissions.insert(Permission::Users(UsersPermission::UsersView));
            permissions.insert(Permission::Users(UsersPermission::UsersViewStudents));
            permissions.insert(Permission::Users(UsersPermission::UsersViewTeachers));
            permissions.insert(Permission::Users(UsersPermission::UsersViewStaff));
            permissions.insert(Permission::Users(UsersPermission::UsersViewAdmins));
            permissions.insert(Permission::Users(UsersPermission::UsersExport));
            permissions.insert(Permission::Users(UsersPermission::UsersBulkDelete));
            permissions.insert(Permission::Users(UsersPermission::UsersAdmissionRegister));
            permissions.insert(Permission::Users(UsersPermission::UsersTransferCertificate));
            permissions.insert(Permission::Users(UsersPermission::UsersCertificateTypes));
            permissions.insert(Permission::Users(UsersPermission::UsersCreateAdmin));
            permissions.insert(Permission::Users(UsersPermission::UsersBulkUpload));

            permissions.insert(Permission::Academics(AcademicsPermission::AcademicsView));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsManageYears,
            ));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsManageClasses,
            ));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsManageSubjects,
            ));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsAssignStudents,
            ));

            permissions.insert(Permission::Reports(ReportsPermission::ReportsView));
            permissions.insert(Permission::Observe(ObservePermission::ObserveView));
            permissions.insert(Permission::Admissions(
                AdmissionsPermission::AdmissionsManage,
            ));
            permissions.insert(Permission::Modules(ModulesPermission::ModulesAccess));

            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsView,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsConfig,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsPermissions,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsBackup,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsReports,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsGraphics,
            ));
            return permissions;
        }

        // --------------------------------------------------------------------
        // 2. School Leadership & Administration
        // (Principal, Headmaster, Chairman, School Admin, Vice Principal)
        // --------------------------------------------------------------------
        if roles.contains("SCHOOL_ADMIN")
            || roles.contains("HEADMASTER")
            || roles.contains("PRINCIPAL")
            || roles.contains("CHAIRMAN")
            || roles.contains("DIRECTOR")
        {
            permissions.insert(Permission::Users(UsersPermission::UsersView));
            permissions.insert(Permission::Users(UsersPermission::UsersViewStudents));
            permissions.insert(Permission::Users(UsersPermission::UsersViewTeachers));
            permissions.insert(Permission::Users(UsersPermission::UsersViewStaff));
            permissions.insert(Permission::Users(UsersPermission::UsersViewAdmins));
            permissions.insert(Permission::Users(UsersPermission::UsersExport));
            permissions.insert(Permission::Users(UsersPermission::UsersBulkDelete));
            permissions.insert(Permission::Users(UsersPermission::UsersAdmissionRegister));
            permissions.insert(Permission::Users(UsersPermission::UsersTransferCertificate));
            permissions.insert(Permission::Users(UsersPermission::UsersCertificateTypes));
            permissions.insert(Permission::Users(UsersPermission::UsersCreateAdmin));
            permissions.insert(Permission::Users(UsersPermission::UsersBulkUpload));

            permissions.insert(Permission::Academics(AcademicsPermission::AcademicsView));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsManageYears,
            ));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsManageClasses,
            ));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsManageSubjects,
            ));
            permissions.insert(Permission::Academics(
                AcademicsPermission::AcademicsAssignStudents,
            ));

            permissions.insert(Permission::Reports(ReportsPermission::ReportsView));
            permissions.insert(Permission::Observe(ObservePermission::ObserveView));
            permissions.insert(Permission::Admissions(
                AdmissionsPermission::AdmissionsManage,
            ));
            permissions.insert(Permission::Modules(ModulesPermission::ModulesAccess));

            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsView,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsConfig,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsPermissions,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsBackup,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsReports,
            ));
            permissions.insert(Permission::SchoolSettings(
                SchoolSettingsPermission::SchoolSettingsGraphics,
            ));
        }

        // --------------------------------------------------------------------
        // 3. Teachers & Academic Faculty
        // (Vice Principal, Class Teacher, Subject Teacher, HOD, Exam Coordinator)
        // --------------------------------------------------------------------
        if roles.contains("TEACHER")
            || roles.contains("CLASS_TEACHER")
            || roles.contains("VICE_PRINCIPAL")
            || roles.contains("HOD")
            || roles.contains("EXAM_COORDINATOR")
            || roles.contains("ACADEMIC_COORDINATOR")
        {
            permissions.insert(Permission::Users(UsersPermission::UsersView));
            permissions.insert(Permission::Users(UsersPermission::UsersViewStudents));
            permissions.insert(Permission::Users(UsersPermission::UsersExport));
            permissions.insert(Permission::Users(UsersPermission::UsersTransferCertificate));

            permissions.insert(Permission::Academics(AcademicsPermission::AcademicsView));
            permissions.insert(Permission::Reports(ReportsPermission::ReportsView));
            permissions.insert(Permission::Observe(ObservePermission::ObserveView));
            permissions.insert(Permission::Modules(ModulesPermission::ModulesAccess));
        }

        // --------------------------------------------------------------------
        // 4. Administrative & Front Desk Staff
        // (Clerk, Accountant, Receptionist, Librarian)
        // --------------------------------------------------------------------
        if roles.contains("STAFF")
            || roles.contains("CLERK")
            || roles.contains("RECEPTIONIST")
            || roles.contains("ACCOUNTANT")
            || roles.contains("LIBRARIAN")
        {
            permissions.insert(Permission::Users(UsersPermission::UsersView));
            permissions.insert(Permission::Users(UsersPermission::UsersViewStudents));
            permissions.insert(Permission::Users(UsersPermission::UsersExport));
            permissions.insert(Permission::Users(UsersPermission::UsersAdmissionRegister));
            permissions.insert(Permission::Users(UsersPermission::UsersTransferCertificate));

            permissions.insert(Permission::Admissions(
                AdmissionsPermission::AdmissionsManage,
            ));
        }

        // --------------------------------------------------------------------
        // 5. Students & Parents (Self-service / Academic view)
        // --------------------------------------------------------------------
        if roles.contains("STUDENT") || roles.contains("PARENT") {
            permissions.insert(Permission::Academics(AcademicsPermission::AcademicsView));
            permissions.insert(Permission::Reports(ReportsPermission::ReportsView));
        }

        permissions
    }

    /// Checks if a user has a specific required permission.
    pub fn can(user: &User, required: Permission) -> bool {
        Self::get_user_permissions(user).contains(&required)
    }

    /// Checks if a user has at least one of the provided permissions.
    pub fn can_any(user: &User, required: &[Permission]) -> bool {
        let perms = Self::get_user_permissions(user);
        required.iter().any(|p| perms.contains(p))
    }

    /// Checks if a user has all of the provided permissions.
    pub fn can_all(user: &User, required: &[Permission]) -> bool {
        let perms = Self::get_user_permissions(user);
        required.iter().all(|p| perms.contains(p))
    }
}
