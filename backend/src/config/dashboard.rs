//! ============================================================================
//! ERAYA DASHBOARD ROLE CONFIGURATION SYSTEM
//! ============================================================================
//!
//! Declarative configuration for role-based dashboard experiences across the
//! institution. Defines tailored metric widgets, quick actions, and focused
//! module cards for each distinct role (Admin, Principal, Teacher, Librarian,
//! Accountant, Clerk, Student, Parent, Transport, etc.).
//! ============================================================================

use crate::modules::auth::backend::User;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardWidget {
    pub id: String,
    pub title: String,
    pub metric_key: String,
    pub icon: String,
    pub description: String,
    pub capability: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardQuickAction {
    pub id: String,
    pub label: String,
    pub path: String,
    pub icon: String,
    pub capability: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardModuleCard {
    pub id: String,
    pub title: String,
    pub description: String,
    pub path: String,
    pub icon: String,
    pub badge: Option<String>,
    pub capability: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDashboardProfile {
    pub role_code: String,
    pub role_name: String,
    pub welcome_subtitle: String,
    pub widgets: Vec<DashboardWidget>,
    pub quick_actions: Vec<DashboardQuickAction>,
    pub module_cards: Vec<DashboardModuleCard>,
    pub restricted_notes: Option<String>,
}

pub struct DashboardConfig;

impl DashboardConfig {
    /// Returns the declarative dashboard profile for a given role code.
    pub fn get_profile_for_role(role_code: &str) -> RoleDashboardProfile {
        let code = role_code.to_uppercase();
        match code.as_str() {
            // ----------------------------------------------------------------
            // 1. PLATFORM SUPER ADMIN
            // ----------------------------------------------------------------
            "SUPER_ADMIN" => RoleDashboardProfile {
                role_code: "SUPER_ADMIN".into(),
                role_name: "Super Administrator".into(),
                welcome_subtitle: "Multi-tenant platform infrastructure & institution management.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "total_schools".into(),
                        title: "Total Schools".into(),
                        metric_key: "total_schools".into(),
                        icon: "building".into(),
                        description: "Registered school tenants".into(),
                        capability: Some("platform:super_admin".into()),
                    },
                    DashboardWidget {
                        id: "platform_users".into(),
                        title: "Total Platform Users".into(),
                        metric_key: "total_platform_users".into(),
                        icon: "users".into(),
                        description: "Global cross-tenant accounts".into(),
                        capability: Some("platform:super_admin".into()),
                    },
                    DashboardWidget {
                        id: "active_sessions".into(),
                        title: "Active Sessions".into(),
                        metric_key: "active_sessions".into(),
                        icon: "activity".into(),
                        description: "Real-time logged-in users".into(),
                        capability: Some("platform:super_admin".into()),
                    },
                    DashboardWidget {
                        id: "system_health".into(),
                        title: "System Health".into(),
                        metric_key: "system_health".into(),
                        icon: "shield".into(),
                        description: "All services operational".into(),
                        capability: Some("platform:super_admin".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "new_school".into(),
                        label: "Provision New School".into(),
                        path: "/dashboard/super-admin/schools/new".into(),
                        icon: "plus".into(),
                        capability: Some("platform:super_admin".into()),
                    },
                    DashboardQuickAction {
                        id: "manage_tenants".into(),
                        label: "Manage Tenants".into(),
                        path: "/dashboard/super-admin".into(),
                        icon: "building".into(),
                        capability: Some("platform:super_admin".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "school_tenants".into(),
                        title: "School Tenants".into(),
                        description: "Provision, configure, and monitor school instances.".into(),
                        path: "/dashboard/super-admin".into(),
                        icon: "building".into(),
                        badge: None,
                        capability: Some("platform:super_admin".into()),
                    },
                ],
                restricted_notes: None,
            },

            // ----------------------------------------------------------------
            // 2. SCHOOL ADMIN & LEADERSHIP (Full School Governance)
            // ----------------------------------------------------------------
            "SCHOOL_ADMIN" | "PRINCIPAL" | "HEADMASTER" | "CHAIRMAN" | "DIRECTOR" => RoleDashboardProfile {
                role_code: "SCHOOL_ADMIN".into(),
                role_name: "School Leadership".into(),
                welcome_subtitle: "Here is an executive overview of your school operations today.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "total_students".into(),
                        title: "Total Students".into(),
                        metric_key: "total_students".into(),
                        icon: "student".into(),
                        description: "Active student directory".into(),
                        capability: Some("users:view_directory".into()),
                    },
                    DashboardWidget {
                        id: "active_classes".into(),
                        title: "Active Classes".into(),
                        metric_key: "active_classes".into(),
                        icon: "classes".into(),
                        description: "Class & section divisions".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardWidget {
                        id: "total_staff".into(),
                        title: "Total Staff".into(),
                        metric_key: "total_staff".into(),
                        icon: "staff".into(),
                        description: "Teachers & administration".into(),
                        capability: Some("users:view_directory".into()),
                    },
                    DashboardWidget {
                        id: "pending_enrollments".into(),
                        title: "Pending Enrollments".into(),
                        metric_key: "pending_enrollments".into(),
                        icon: "hourglass".into(),
                        description: "Unassigned student records".into(),
                        capability: Some("academics:assign_students".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "add_student".into(),
                        label: "Register Student".into(),
                        path: "/dashboard/users?action=admission_register".into(),
                        icon: "user_plus".into(),
                        capability: Some("users:admission_register".into()),
                    },
                    DashboardQuickAction {
                        id: "manage_classes".into(),
                        label: "Class Divisions".into(),
                        path: "/dashboard/academics/classes".into(),
                        icon: "layout".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardQuickAction {
                        id: "export_data".into(),
                        label: "Export Records".into(),
                        path: "/dashboard/users?action=export".into(),
                        icon: "export".into(),
                        capability: Some("users:export".into()),
                    },
                    DashboardQuickAction {
                        id: "school_settings".into(),
                        label: "School Settings".into(),
                        path: "/dashboard/school-setting".into(),
                        icon: "config".into(),
                        capability: Some("settings:view".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "user_directory".into(),
                        title: "User Directory".into(),
                        description: "Manage students, teachers, parents, and administrative staff.".into(),
                        path: "/dashboard/users".into(),
                        icon: "users".into(),
                        badge: None,
                        capability: Some("users:view_directory".into()),
                    },
                    DashboardModuleCard {
                        id: "academics_hub".into(),
                        title: "Academics Hub".into(),
                        description: "Configure timetables, subjects, classes, and teacher assignments.".into(),
                        path: "/dashboard/academics".into(),
                        icon: "book".into(),
                        badge: None,
                        capability: Some("academics:view".into()),
                    },
                    DashboardModuleCard {
                        id: "reports_analytics".into(),
                        title: "Reports & Analytics".into(),
                        description: "Attendance trends, examination scorecards, and audit logs.".into(),
                        path: "/dashboard/reports".into(),
                        icon: "reports".into(),
                        badge: None,
                        capability: Some("settings:view".into()),
                    },
                ],
                restricted_notes: None,
            },

            // ----------------------------------------------------------------
            // 3. TEACHER & CLASS TEACHER (Academics & Classroom Focus)
            // ----------------------------------------------------------------
            "TEACHER" | "CLASS_TEACHER" | "SUBJECT_COORDINATOR" | "HOD" => RoleDashboardProfile {
                role_code: "TEACHER".into(),
                role_name: "Academic Faculty".into(),
                welcome_subtitle: "Manage your assigned classes, subject timetables, and student assessments.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "assigned_classes".into(),
                        title: "Assigned Classes".into(),
                        metric_key: "assigned_classes".into(),
                        icon: "classes".into(),
                        description: "Classes under your instruction".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardWidget {
                        id: "todays_periods".into(),
                        title: "Today's Periods".into(),
                        metric_key: "todays_periods".into(),
                        icon: "calendar".into(),
                        description: "Scheduled timetable lectures".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardWidget {
                        id: "my_students".into(),
                        title: "My Students".into(),
                        metric_key: "my_students".into(),
                        icon: "student".into(),
                        description: "Total students across assigned sections".into(),
                        capability: Some("users:view_directory".into()),
                    },
                    DashboardWidget {
                        id: "pending_grades".into(),
                        title: "Pending Evaluations".into(),
                        metric_key: "pending_grades".into(),
                        icon: "edit".into(),
                        description: "Marks awaiting submission".into(),
                        capability: Some("exams:enter_marks".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "mark_attendance".into(),
                        label: "Take Attendance".into(),
                        path: "/dashboard/attendance".into(),
                        icon: "check_circle".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardQuickAction {
                        id: "view_timetable".into(),
                        label: "My Timetable".into(),
                        path: "/dashboard/academics/timetable".into(),
                        icon: "calendar".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardQuickAction {
                        id: "enter_marks".into(),
                        label: "Enter Marks".into(),
                        path: "/dashboard/academics/marks".into(),
                        icon: "edit".into(),
                        capability: Some("exams:enter_marks".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "academics_timetable".into(),
                        title: "Weekly Timetable".into(),
                        description: "View and verify assigned lecture schedules and period timings.".into(),
                        path: "/dashboard/academics/timetable".into(),
                        icon: "calendar".into(),
                        badge: None,
                        capability: Some("academics:view".into()),
                    },
                    DashboardModuleCard {
                        id: "student_roster".into(),
                        title: "Class Roster".into(),
                        description: "View student directory for your assigned classes and sections.".into(),
                        path: "/dashboard/users?role=STUDENT".into(),
                        icon: "student".into(),
                        badge: None,
                        capability: Some("users:view_directory".into()),
                    },
                ],
                restricted_notes: Some("Administrative billing, staff payroll, and system security configurations are restricted.".into()),
            },

            // ----------------------------------------------------------------
            // 4. LIBRARIAN (Library & Book Circulation Focus)
            // ----------------------------------------------------------------
            "LIBRARIAN" | "ASST_LIBRARIAN" => RoleDashboardProfile {
                role_code: "LIBRARIAN".into(),
                role_name: "Library Desk".into(),
                welcome_subtitle: "Track book issues, returns, inventory catalogue, and student reading records.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "total_books".into(),
                        title: "Total Catalog Titles".into(),
                        metric_key: "total_books".into(),
                        icon: "book".into(),
                        description: "Books in library registry".into(),
                        capability: Some("library:manage_catalogue".into()),
                    },
                    DashboardWidget {
                        id: "books_issued".into(),
                        title: "Books Currently Issued".into(),
                        metric_key: "books_issued".into(),
                        icon: "export".into(),
                        description: "Active circulations".into(),
                        capability: Some("library:issue_return".into()),
                    },
                    DashboardWidget {
                        id: "overdue_returns".into(),
                        title: "Overdue Returns".into(),
                        metric_key: "overdue_returns".into(),
                        icon: "alert_circle".into(),
                        description: "Late returns needing fine/reminder".into(),
                        capability: Some("library:issue_return".into()),
                    },
                    DashboardWidget {
                        id: "daily_footfall".into(),
                        title: "Today's Checkouts".into(),
                        metric_key: "daily_footfall".into(),
                        icon: "activity".into(),
                        description: "Circulation desk activity today".into(),
                        capability: Some("library:issue_return".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "issue_book".into(),
                        label: "Issue Book".into(),
                        path: "/dashboard/library/issue".into(),
                        icon: "plus".into(),
                        capability: Some("library:issue_return".into()),
                    },
                    DashboardQuickAction {
                        id: "return_book".into(),
                        label: "Process Return".into(),
                        path: "/dashboard/library/return".into(),
                        icon: "check_circle".into(),
                        capability: Some("library:issue_return".into()),
                    },
                    DashboardQuickAction {
                        id: "search_catalogue".into(),
                        label: "Search Catalog".into(),
                        path: "/dashboard/library/catalog".into(),
                        icon: "search".into(),
                        capability: Some("library:manage_catalogue".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "library_circulation".into(),
                        title: "Circulation Desk".into(),
                        description: "Issue, return, and renew books for students and faculty.".into(),
                        path: "/dashboard/library/circulation".into(),
                        icon: "book".into(),
                        badge: None,
                        capability: Some("library:issue_return".into()),
                    },
                    DashboardModuleCard {
                        id: "book_catalogue".into(),
                        title: "Book Catalog & ISBN".into(),
                        description: "Add new acquisitions, barcode tags, and category shelves.".into(),
                        path: "/dashboard/library/catalog".into(),
                        icon: "grid".into(),
                        badge: None,
                        capability: Some("library:manage_catalogue".into()),
                    },
                ],
                restricted_notes: Some("Academic grading, fee structures, and user credential management are restricted.".into()),
            },

            // ----------------------------------------------------------------
            // 5. ACCOUNTANT & OFFICE (Fees, Billing & Accounts Focus)
            // ----------------------------------------------------------------
            "ACCOUNTANT" | "BURSAR" | "FEE_CLERK" | "CASHIER" => RoleDashboardProfile {
                role_code: "ACCOUNTANT".into(),
                role_name: "Finance & Accounts".into(),
                welcome_subtitle: "Manage student fee collection, dues, receipts, and expense accounts.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "fee_collected_today".into(),
                        title: "Fee Collected Today".into(),
                        metric_key: "fee_collected_today".into(),
                        icon: "credit_card".into(),
                        description: "Total cashier collections".into(),
                        capability: Some("finance:collect_fees".into()),
                    },
                    DashboardWidget {
                        id: "pending_fee_dues".into(),
                        title: "Pending Fee Dues".into(),
                        metric_key: "pending_fee_dues".into(),
                        icon: "alert_circle".into(),
                        description: "Outstanding balances this term".into(),
                        capability: Some("finance:view_reports".into()),
                    },
                    DashboardWidget {
                        id: "receipts_issued".into(),
                        title: "Receipts Issued".into(),
                        metric_key: "receipts_issued".into(),
                        icon: "receipt".into(),
                        description: "Official payment vouchers".into(),
                        capability: Some("finance:collect_fees".into()),
                    },
                    DashboardWidget {
                        id: "students_with_dues".into(),
                        title: "Students with Dues".into(),
                        metric_key: "students_with_dues".into(),
                        icon: "student".into(),
                        description: "Accounts requiring follow-up".into(),
                        capability: Some("finance:view_reports".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "collect_fee".into(),
                        label: "Collect Fee".into(),
                        path: "/dashboard/finance/collect".into(),
                        icon: "credit_card".into(),
                        capability: Some("finance:collect_fees".into()),
                    },
                    DashboardQuickAction {
                        id: "print_receipt".into(),
                        label: "Generate Receipt".into(),
                        path: "/dashboard/finance/receipts".into(),
                        icon: "receipt".into(),
                        capability: Some("finance:collect_fees".into()),
                    },
                    DashboardQuickAction {
                        id: "fee_structure".into(),
                        label: "Fee Breakdown".into(),
                        path: "/dashboard/finance/structure".into(),
                        icon: "grid".into(),
                        capability: Some("finance:manage_fee_structure".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "fee_counter".into(),
                        title: "Fee Collection Counter".into(),
                        description: "Process cash, UPI, card, and net-banking term fee payments.".into(),
                        path: "/dashboard/finance/collect".into(),
                        icon: "credit_card".into(),
                        badge: None,
                        capability: Some("finance:collect_fees".into()),
                    },
                    DashboardModuleCard {
                        id: "financial_reports".into(),
                        title: "Daily Ledger & Reports".into(),
                        description: "Daily reconciliation, fee dues lists, and bank deposit statements.".into(),
                        path: "/dashboard/finance/reports".into(),
                        icon: "reports".into(),
                        badge: None,
                        capability: Some("finance:view_reports".into()),
                    },
                ],
                restricted_notes: Some("Academic curriculum, exam marks entry, and system settings are restricted.".into()),
            },

            // ----------------------------------------------------------------
            // 6. ADMISSION COUNSELOR & OFFICE CLERK (Enrollment & Desk Focus)
            // ----------------------------------------------------------------
            "ADMISSION_COUNSELOR" | "FRONT_DESK_EXECUTIVE" | "CLERK" | "RECEPTIONIST" => RoleDashboardProfile {
                role_code: "CLERK".into(),
                role_name: "Administrative Office".into(),
                welcome_subtitle: "Handle student admission inquiries, document verification, and certificates.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "inquiries_today".into(),
                        title: "New Inquiries".into(),
                        metric_key: "inquiries_today".into(),
                        icon: "user_plus".into(),
                        description: "Admissions pipeline entries".into(),
                        capability: Some("users:admission_register".into()),
                    },
                    DashboardWidget {
                        id: "tc_requests".into(),
                        title: "TC Applications".into(),
                        metric_key: "tc_requests".into(),
                        icon: "certificate".into(),
                        description: "Transfer certificate requests".into(),
                        capability: Some("certificate:issue_tc".into()),
                    },
                    DashboardWidget {
                        id: "total_students".into(),
                        title: "Active Students".into(),
                        metric_key: "total_students".into(),
                        icon: "student".into(),
                        description: "Current enrolled strength".into(),
                        capability: Some("users:view_directory".into()),
                    },
                    DashboardWidget {
                        id: "pending_docs".into(),
                        title: "Pending Documents".into(),
                        metric_key: "pending_docs".into(),
                        icon: "document".into(),
                        description: "Incomplete student dossiers".into(),
                        capability: Some("users:admission_register".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "new_admission".into(),
                        label: "New Admission".into(),
                        path: "/dashboard/users?action=admission_register".into(),
                        icon: "user_plus".into(),
                        capability: Some("users:admission_register".into()),
                    },
                    DashboardQuickAction {
                        id: "issue_tc".into(),
                        label: "Issue TC".into(),
                        path: "/dashboard/users?action=transfer_certificate".into(),
                        icon: "certificate".into(),
                        capability: Some("certificate:issue_tc".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "admission_registry".into(),
                        title: "Admissions Registry".into(),
                        description: "Process new enrollments, assign admission numbers, and generate ID cards.".into(),
                        path: "/dashboard/users?action=admission_register".into(),
                        icon: "user_plus".into(),
                        badge: None,
                        capability: Some("users:admission_register".into()),
                    },
                    DashboardModuleCard {
                        id: "certificate_desk".into(),
                        title: "Certificates & TC Desk".into(),
                        description: "Issue Transfer, Bonafide, Study, and Character certificates.".into(),
                        path: "/dashboard/users?action=transfer_certificate".into(),
                        icon: "certificate".into(),
                        badge: None,
                        capability: Some("certificate:issue_tc".into()),
                    },
                ],
                restricted_notes: Some("Academic exam creation and system server configurations are restricted.".into()),
            },

            // ----------------------------------------------------------------
            // 7. TRANSPORT MANAGER
            // ----------------------------------------------------------------
            "TRANSPORT_MANAGER" | "BUS_IN_CHARGE" | "DRIVER" => RoleDashboardProfile {
                role_code: "TRANSPORT".into(),
                role_name: "Transport & Fleet".into(),
                welcome_subtitle: "Manage school bus routes, driver rosters, vehicle tracking, and student pickups.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "active_vehicles".into(),
                        title: "Active Buses".into(),
                        metric_key: "active_vehicles".into(),
                        icon: "truck".into(),
                        description: "Vehicles currently on routes".into(),
                        capability: Some("transport:manage_fleet".into()),
                    },
                    DashboardWidget {
                        id: "transport_students".into(),
                        title: "Bus Commuters".into(),
                        metric_key: "transport_students".into(),
                        icon: "student".into(),
                        description: "Students assigned to bus routes".into(),
                        capability: Some("transport:manage_routes".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "manage_routes".into(),
                        label: "Bus Routes".into(),
                        path: "/dashboard/transport/routes".into(),
                        icon: "map".into(),
                        capability: Some("transport:manage_routes".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "fleet_management".into(),
                        title: "Fleet & Routes".into(),
                        description: "Manage route stops, driver schedules, and bus maintenance records.".into(),
                        path: "/dashboard/transport/routes".into(),
                        icon: "truck".into(),
                        badge: None,
                        capability: Some("transport:manage_routes".into()),
                    },
                ],
                restricted_notes: Some("Finance, academic grading, and student academic records are restricted.".into()),
            },

            // ----------------------------------------------------------------
            // 8. STUDENT & PARENT (Self-Service Portal)
            // ----------------------------------------------------------------
            "STUDENT" | "PARENT" => RoleDashboardProfile {
                role_code: "STUDENT".into(),
                role_name: "Student & Parent Portal".into(),
                welcome_subtitle: "Track your academic timetable, attendance records, exam results, and school notices.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "attendance_percent".into(),
                        title: "Attendance Rate".into(),
                        metric_key: "attendance_percent".into(),
                        icon: "check_circle".into(),
                        description: "Overall academic attendance".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardWidget {
                        id: "upcoming_exams".into(),
                        title: "Upcoming Tests".into(),
                        metric_key: "upcoming_exams".into(),
                        icon: "calendar".into(),
                        description: "Scheduled assessments".into(),
                        capability: Some("academics:view".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "view_timetable".into(),
                        label: "My Timetable".into(),
                        path: "/dashboard/academics/timetable".into(),
                        icon: "calendar".into(),
                        capability: Some("academics:view".into()),
                    },
                    DashboardQuickAction {
                        id: "view_report_card".into(),
                        label: "Report Card".into(),
                        path: "/dashboard/reports".into(),
                        icon: "document".into(),
                        capability: Some("academics:view".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "academic_schedule".into(),
                        title: "Class Schedule & Subjects".into(),
                        description: "Check daily period timings and homework updates.".into(),
                        path: "/dashboard/academics".into(),
                        icon: "book".into(),
                        badge: None,
                        capability: Some("academics:view".into()),
                    },
                ],
                restricted_notes: Some("Administrative and management functions are restricted.".into()),
            },

            // ----------------------------------------------------------------
            // DEFAULT FALLBACK (Standard Staff Profile)
            // ----------------------------------------------------------------
            _ => RoleDashboardProfile {
                role_code: "STAFF".into(),
                role_name: "School Staff".into(),
                welcome_subtitle: "Here is your operational overview for today.".into(),
                widgets: vec![
                    DashboardWidget {
                        id: "total_students".into(),
                        title: "Total Students".into(),
                        metric_key: "total_students".into(),
                        icon: "student".into(),
                        description: "Active student directory".into(),
                        capability: Some("users:view_directory".into()),
                    },
                ],
                quick_actions: vec![
                    DashboardQuickAction {
                        id: "view_users".into(),
                        label: "User Directory".into(),
                        path: "/dashboard/users".into(),
                        icon: "users".into(),
                        capability: Some("users:view_directory".into()),
                    },
                ],
                module_cards: vec![
                    DashboardModuleCard {
                        id: "users_module".into(),
                        title: "Directory".into(),
                        description: "View institution directories and contacts.".into(),
                        path: "/dashboard/users".into(),
                        icon: "users".into(),
                        badge: None,
                        capability: Some("users:view_directory".into()),
                    },
                ],
                restricted_notes: None,
            },
        }
    }

    /// Evaluates the logged-in user and builds their personalized dashboard configuration
    /// based on their highest priority role and assigned capabilities.
    pub fn build_user_dashboard(user: &User) -> RoleDashboardProfile {
        let is_super_admin = user
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"));
        let is_school_admin = user.is_school_admin();

        // Determine primary profile role
        let primary_role = if is_super_admin {
            "SUPER_ADMIN"
        } else if is_school_admin {
            "SCHOOL_ADMIN"
        } else if let Some(role) = user.roles.first() {
            role.as_str()
        } else {
            "STAFF"
        };

        let mut profile = Self::get_profile_for_role(primary_role);

        let can_access = |cap_opt: &Option<String>| -> bool {
            match cap_opt {
                None => true,
                Some(cap) => {
                    if is_super_admin || is_school_admin {
                        return true;
                    }
                    user.has_capability(cap)
                }
            }
        };

        // Filter widgets, actions, and module cards based on capabilities
        profile.widgets.retain(|w| can_access(&w.capability));
        profile.quick_actions.retain(|a| can_access(&a.capability));
        profile.module_cards.retain(|m| can_access(&m.capability));

        profile
    }

    /// Returns the declarative dashboard profile for the Academics Module Home
    pub fn get_academics_dashboard(user: &User) -> AcademicsDashboardProfile {
        let is_super_admin = user
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"));
        let is_school_admin = user.is_school_admin();

        let can_access = |cap_opt: &Option<String>| -> bool {
            match cap_opt {
                None => true,
                Some(cap) => {
                    if is_super_admin || is_school_admin {
                        return true;
                    }
                    user.has_capability(cap)
                }
            }
        };

        let mut profile = AcademicsDashboardProfile {
            title: "Academics Overview".into(),
            subtitle:
                "Manage curriculum, class section capacities, student assignments, and schedules."
                    .into(),
            widgets: vec![
                DashboardWidget {
                    id: "active_classes".into(),
                    title: "Active Classes".into(),
                    metric_key: "active_classes".into(),
                    icon: "classes".into(),
                    description: "Configured class & section divisions".into(),
                    capability: Some("academics:view".into()),
                },
                DashboardWidget {
                    id: "total_subjects".into(),
                    title: "Configured Subjects".into(),
                    metric_key: "total_subjects".into(),
                    icon: "book".into(),
                    description: "Core, elective & vocational subjects".into(),
                    capability: Some("academics:view".into()),
                },
                DashboardWidget {
                    id: "total_enrolled".into(),
                    title: "Assigned Students".into(),
                    metric_key: "total_students".into(),
                    icon: "student".into(),
                    description: "Students assigned to class sections".into(),
                    capability: Some("academics:view".into()),
                },
                DashboardWidget {
                    id: "total_capacity".into(),
                    title: "Total Capacity".into(),
                    metric_key: "total_capacity".into(),
                    icon: "activity".into(),
                    description: "Total seats across class sections".into(),
                    capability: Some("academics:view".into()),
                },
            ],
            quick_actions: vec![
                DashboardQuickAction {
                    id: "add_subject".into(),
                    label: "Add Subject".into(),
                    path: "/dashboard/academics?tab=subjects".into(),
                    icon: "book".into(),
                    capability: Some("academics:view".into()),
                },
                DashboardQuickAction {
                    id: "manage_classes".into(),
                    label: "Class Capacity".into(),
                    path: "/dashboard/academics?tab=classes".into(),
                    icon: "layout".into(),
                    capability: Some("academics:view".into()),
                },
                DashboardQuickAction {
                    id: "assign_students".into(),
                    label: "Assign Students".into(),
                    path: "/dashboard/academics?tab=assignment".into(),
                    icon: "user_plus".into(),
                    capability: Some("academics:assign_students".into()),
                },
                DashboardQuickAction {
                    id: "view_timetable".into(),
                    label: "Timetable".into(),
                    path: "/dashboard/academics?tab=timetable".into(),
                    icon: "calendar".into(),
                    capability: Some("academics:view".into()),
                },
            ],
            module_cards: vec![
                DashboardModuleCard {
                    id: "subjects_hub".into(),
                    title: "Subjects & Curriculum".into(),
                    description: "Configure core subjects, elective courses, and syllabi codes."
                        .into(),
                    path: "/dashboard/academics?tab=subjects".into(),
                    icon: "book".into(),
                    badge: None,
                    capability: Some("academics:view".into()),
                },
                DashboardModuleCard {
                    id: "classes_hub".into(),
                    title: "Classes & Section Capacities".into(),
                    description:
                        "Monitor section enrollments, student caps, and division headcounts.".into(),
                    path: "/dashboard/academics?tab=classes".into(),
                    icon: "layout".into(),
                    badge: None,
                    capability: Some("academics:view".into()),
                },
                DashboardModuleCard {
                    id: "assignment_hub".into(),
                    title: "Student Class Assigner".into(),
                    description:
                        "Bulk assign student batches to class sections by admission number.".into(),
                    path: "/dashboard/academics?tab=assignment".into(),
                    icon: "student".into(),
                    badge: None,
                    capability: Some("academics:assign_students".into()),
                },
                DashboardModuleCard {
                    id: "timetable_hub".into(),
                    title: "Timetable & Periods".into(),
                    description: "Master schedules, classroom allocations, and teaching periods."
                        .into(),
                    path: "/dashboard/academics?tab=timetable".into(),
                    icon: "calendar".into(),
                    badge: Some("Beta".into()),
                    capability: Some("academics:view".into()),
                },
                DashboardModuleCard {
                    id: "class_teacher_hub".into(),
                    title: "Class Teachers & Mentors".into(),
                    description:
                        "Assign faculty mentors and in-charge teachers to class divisions.".into(),
                    path: "/dashboard/academics?tab=class-teacher".into(),
                    icon: "teacher".into(),
                    badge: None,
                    capability: Some("academics:view".into()),
                },
            ],
        };

        profile.widgets.retain(|w| can_access(&w.capability));
        profile.quick_actions.retain(|a| can_access(&a.capability));
        profile.module_cards.retain(|m| can_access(&m.capability));

        profile
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcademicsDashboardProfile {
    pub title: String,
    pub subtitle: String,
    pub widgets: Vec<DashboardWidget>,
    pub quick_actions: Vec<DashboardQuickAction>,
    pub module_cards: Vec<DashboardModuleCard>,
}
