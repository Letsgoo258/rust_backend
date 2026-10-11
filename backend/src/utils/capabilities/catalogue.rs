/// Master catalogue of all standard capabilities supported across the platform.
pub fn catalogue() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        // 1. Bulk Edit & Multi-Field Updates
        (
            "bulk:edit_multiple_fields",
            "Edit Multiple Fields Together",
            "Bulk Edit Settings",
            "Allows dragging and editing multiple columns and rows simultaneously in Admission Register",
        ),
        (
            "bulk:delete",
            "Bulk Delete Rows",
            "Bulk Edit Settings",
            "Allows multi-row deletion across users directory",
        ),
        (
            "users:admission_register",
            "Admission Register Studio",
            "Bulk Edit Settings",
            "Full access to inline spreadsheet register for student and staff data",
        ),

        // 2. Export & Certificates
        (
            "users:export",
            "Export User Data",
            "Exports & Documents",
            "Export school data to CSV, Excel spreadsheet, or PDF format",
        ),
        (
            "certificate:issue_tc",
            "Issue Transfer Certificate",
            "Exports & Documents",
            "Preview, customize, and issue formal Transfer Certificates with seal",
        ),
        (
            "certificate:manage_types",
            "Manage Certificate Types",
            "Exports & Documents",
            "Configure Bonafide, Conduct, and Study certificate templates",
        ),

        // 3. User Management & Directory
        (
            "users:view_directory",
            "View Users Directory",
            "User Management",
            "View and filter student, teacher, staff, and admin profiles",
        ),
        (
            "users:create_admin",
            "Create School Admins",
            "User Management",
            "Add new administrative users with elevated privileges",
        ),
        (
            "users:bulk_onboard",
            "Bulk Onboard Users",
            "User Management",
            "Strictly School Admin: Upload students and staff via CSV onboarding queues",
        ),

        // 4. Academics & Curriculum
        (
            "academics:view",
            "View Academics",
            "Academics",
            "Inspect academic classes, sections, and subjects",
        ),
        (
            "academics:manage_curriculum",
            "Manage Classes & Subjects",
            "Academics",
            "Create, edit, and reorganize class levels, sections, and subjects",
        ),
        (
            "academics:assign_students",
            "Assign Students to Classes",
            "Academics",
            "Perform bulk student allocation to class sections",
        ),
        (
            "academics:manage_timetable",
            "Manage Timetable & Schedule",
            "Academics",
            "Configure class scheduling rules, periods, and timetable dispatch",
        ),

        // 5. Examinations & Grading
        (
            "exams:manage_schedules",
            "Manage Exam Schedules",
            "Examinations",
            "Create and dispatch term exam schedules and test timetables",
        ),
        (
            "exams:enter_marks",
            "Enter & Grade Marks",
            "Examinations",
            "Enter student subject marks, grade reports, and assessments",
        ),

        // 6. Fees & Accounts
        (
            "finance:view_fees",
            "View Fee Collections",
            "Finance & Fees",
            "View student fee structures, challans, and payment dues",
        ),
        (
            "finance:collect_fees",
            "Collect Fees & Receipts",
            "Finance & Fees",
            "Accept tuition/transport fees, generate receipts and invoices",
        ),

        // 7. Transport & Logistics
        (
            "transport:manage_routes",
            "Manage Bus Routes & Stops",
            "Transport & Logistics",
            "Configure bus fleet routes, pickup stops, and vehicle assignments",
        ),
        (
            "transport:view_roster",
            "View Bus Student Rosters",
            "Transport & Logistics",
            "Access bus passenger rosters and emergency contact numbers",
        ),

        // 8. Library & Resources
        (
            "library:manage_catalog",
            "Manage Library Catalog",
            "Library",
            "Issue, return, catalog, and track books and digital resources",
        ),

        // 9. School Administration & Settings
        (
            "settings:view",
            "View School Settings",
            "School Administration",
            "Access school configuration, backups, reports, and permissions",
        ),
        (
            "settings:configure_roles",
            "Configure Role Permissions",
            "School Administration",
            "Customize capability toggles for individual roles",
        ),
        (
            "settings:manage_backups",
            "Manage School Backups",
            "School Administration",
            "Create, download, and restore school database backups",
        ),
    ]
}
