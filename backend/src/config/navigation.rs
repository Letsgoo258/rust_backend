use crate::modules::auth::backend::User;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationItem {
    pub name: String,
    pub path: String,
    pub icon: String,
    pub capability: Option<String>,
    pub badge: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationSection {
    pub section: String,
    pub items: Vec<NavigationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationTree {
    pub top_nav: Vec<NavigationItem>,
    pub users_sidebar: Vec<NavigationSection>,
    pub academics_sidebar: Vec<NavigationSection>,
    pub school_settings_sidebar: Vec<NavigationSection>,
    pub platform_sidebar: Vec<NavigationSection>,
    pub modules_sidebar: Vec<NavigationSection>,
}

pub struct NavigationConfig;

impl NavigationConfig {
    /// Return the master platform navigation structure
    pub fn get_master_tree() -> NavigationTree {
        NavigationTree {
            top_nav: vec![
                NavigationItem {
                    name: "Eravaya Home".into(),
                    path: "/dashboard".into(),
                    icon: "home".into(),
                    capability: None,
                    badge: None,
                },
                NavigationItem {
                    name: "Platform Admin".into(),
                    path: "/dashboard/super-admin".into(),
                    icon: "shield".into(),
                    capability: Some("platform:super_admin".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Users".into(),
                    path: "/dashboard/users".into(),
                    icon: "users".into(),
                    capability: Some("users:view_directory".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Attendance".into(),
                    path: "/dashboard/attendance".into(),
                    icon: "calendar".into(),
                    capability: Some("academics:view".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Academics".into(),
                    path: "/dashboard/academics".into(),
                    icon: "book".into(),
                    capability: Some("academics:view".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Reports".into(),
                    path: "/dashboard/reports".into(),
                    icon: "reports".into(),
                    capability: Some("settings:view".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Observe".into(),
                    path: "/dashboard/observe".into(),
                    icon: "eye".into(),
                    capability: Some("settings:view".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Admissions".into(),
                    path: "/dashboard/admissions".into(),
                    icon: "user_plus".into(),
                    capability: Some("users:admission_register".into()),
                    badge: None,
                },
                NavigationItem {
                    name: "Modules".into(),
                    path: "/dashboard/modules".into(),
                    icon: "grid".into(),
                    capability: None,
                    badge: None,
                },
            ],

            academics_sidebar: vec![
                NavigationSection {
                    section: "HOME".into(),
                    items: vec![NavigationItem {
                        name: "Overview".into(),
                        path: "/dashboard/academics".into(),
                        icon: "home".into(),
                        capability: Some("academics:view".into()),
                        badge: None,
                    }],
                },
                NavigationSection {
                    section: "MANAGEMENT".into(),
                    items: vec![
                        NavigationItem {
                            name: "Subjects".into(),
                            path: "/dashboard/academics/subjects".into(),
                            icon: "book".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Classes & Capacities".into(),
                            path: "/dashboard/academics/classes".into(),
                            icon: "layout".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Timetable".into(),
                            path: "/dashboard/academics/timetable".into(),
                            icon: "calendar".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Class Teacher".into(),
                            path: "/dashboard/academics/class-teacher".into(),
                            icon: "teacher".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Substitution".into(),
                            path: "/dashboard/academics/substitution".into(),
                            icon: "calendar".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Duty Search".into(),
                            path: "/dashboard/academics/duty-search".into(),
                            icon: "users".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                    ],
                },
                NavigationSection {
                    section: "TEACHER CORE".into(),
                    items: vec![
                        NavigationItem {
                            name: "Exams".into(),
                            path: "/dashboard/academics/exams".into(),
                            icon: "certificate".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Results".into(),
                            path: "/dashboard/academics/results".into(),
                            icon: "reports".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Homeworks".into(),
                            path: "/dashboard/academics/homeworks".into(),
                            icon: "book".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Assignments".into(),
                            path: "/dashboard/academics/assignments".into(),
                            icon: "spreadsheet".into(),
                            capability: Some("academics:view".into()),
                            badge: None,
                        },
                    ],
                },
                NavigationSection {
                    section: "CBSE MANDATES".into(),
                    items: vec![NavigationItem {
                        name: "CBSE Mandates".into(),
                        path: "/dashboard/academics/cbse-mandates".into(),
                        icon: "certificate".into(),
                        capability: Some("academics:view".into()),
                        badge: None,
                    }],
                },
                NavigationSection {
                    section: "REPORTS".into(),
                    items: vec![NavigationItem {
                        name: "Academic Reports".into(),
                        path: "/dashboard/reports".into(),
                        icon: "reports".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    }],
                },
                NavigationSection {
                    section: "ACTIONS".into(),
                    items: vec![NavigationItem {
                        name: "Bulk Assign Class".into(),
                        path: "/dashboard/academics/bulk-assignment".into(),
                        icon: "user_plus".into(),
                        capability: Some("academics:assign_students".into()),
                        badge: None,
                    }],
                },
            ],
            users_sidebar: vec![
                NavigationSection {
                    section: "ROLES & DIRECTORY".into(),
                    items: vec![
                        NavigationItem {
                            name: "All Users".into(),
                            path: "/dashboard/users".into(),
                            icon: "users".into(),
                            capability: Some("users:view_directory".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Students".into(),
                            path: "/dashboard/users?role=STUDENT".into(),
                            icon: "student".into(),
                            capability: Some("users:view_directory".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Teachers".into(),
                            path: "/dashboard/users?role=TEACHER".into(),
                            icon: "teacher".into(),
                            capability: Some("users:view_directory".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Staffs".into(),
                            path: "/dashboard/users?role=STAFF".into(),
                            icon: "staff".into(),
                            capability: Some("users:view_directory".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Admins".into(),
                            path: "/dashboard/users?role=SCHOOL_ADMIN".into(),
                            icon: "admin_shield".into(),
                            capability: Some("users:create_admin".into()),
                            badge: None,
                        },
                    ],
                },
                NavigationSection {
                    section: "MANAGEMENT & ACTIONS".into(),
                    items: vec![
                        NavigationItem {
                            name: "Add user".into(),
                            path: "/dashboard/users/add".into(),
                            icon: "user_plus".into(),
                            capability: Some("users:bulk_onboard".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Export user data".into(),
                            path: "/dashboard/users?action=export".into(),
                            icon: "export".into(),
                            capability: Some("users:export".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Bulk delete".into(),
                            path: "/dashboard/users?action=bulk_delete".into(),
                            icon: "trash".into(),
                            capability: Some("bulk:delete".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Admission register".into(),
                            path: "/dashboard/users?action=admission_register".into(),
                            icon: "spreadsheet".into(),
                            capability: Some("users:admission_register".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Transfer certificate".into(),
                            path: "/dashboard/users?action=transfer_certificate".into(),
                            icon: "certificate".into(),
                            capability: Some("certificate:issue_tc".into()),
                            badge: None,
                        },
                        NavigationItem {
                            name: "Certificate types".into(),
                            path: "/dashboard/users?action=certificate_types".into(),
                            icon: "certificate_types".into(),
                            capability: Some("certificate:manage_types".into()),
                            badge: None,
                        },
                    ],
                },
            ],
            school_settings_sidebar: vec![NavigationSection {
                section: "SCHOOL SETTINGS".into(),
                items: vec![
                    NavigationItem {
                        name: "School Info".into(),
                        path: "/dashboard/school-setting".into(),
                        icon: "school_info".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Config".into(),
                        path: "/dashboard/school-setting?tab=config".into(),
                        icon: "config".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Assets & Rooms".into(),
                        path: "/dashboard/school-setting?tab=assets-rooms".into(),
                        icon: "grid".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Permissions".into(),
                        path: "/dashboard/school-setting?tab=permissions".into(),
                        icon: "permissions".into(),
                        capability: Some("settings:configure_roles".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Backup".into(),
                        path: "/dashboard/school-setting?tab=backup".into(),
                        icon: "backup".into(),
                        capability: Some("settings:manage_backups".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Reports".into(),
                        path: "/dashboard/school-setting?tab=reports".into(),
                        icon: "reports".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Graphic Materials".into(),
                        path: "/dashboard/school-setting?tab=graphics".into(),
                        icon: "graphics".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "WhatsApp".into(),
                        path: "/dashboard/school-setting?tab=whatsapp".into(),
                        icon: "whatsapp".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Admission".into(),
                        path: "/dashboard/school-setting?tab=admission".into(),
                        icon: "user_add".into(),
                        capability: Some("admission:manage".into()),
                        badge: None,
                    },
                ],
            }],

            modules_sidebar: vec![NavigationSection {
                section: "MODULES".into(),
                items: vec![
                    NavigationItem {
                        name: "Notification Engine".into(),
                        path: "/dashboard/modules/notifications".into(),
                        icon: "graphics".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Library".into(),
                        path: "/dashboard/modules/library".into(),
                        icon: "book".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Transport Manager".into(),
                        path: "/dashboard/modules/transport".into(),
                        icon: "calendar".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "Canteen".into(),
                        path: "/dashboard/modules/canteen".into(),
                        icon: "home".into(),
                        capability: Some("settings:view".into()),
                        badge: None,
                    },
                ],
            }],
            platform_sidebar: vec![NavigationSection {
                section: "PLATFORM".into(),
                items: vec![
                    NavigationItem {
                        name: "Overview".into(),
                        path: "/dashboard/super-admin".into(),
                        icon: "home".into(),
                        capability: Some("platform:super_admin".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "School Tenants".into(),
                        path: "/dashboard/super-admin/schools/new".into(),
                        icon: "school_info".into(),
                        capability: Some("platform:super_admin".into()),
                        badge: None,
                    },
                ],
            }],
        }
    }

    /// Dynamically filter the navigation tree based on user capabilities & roles
    pub fn build_user_navigation(user: &User) -> NavigationTree {
        let master = Self::get_master_tree();
        let is_super_admin = user.roles.iter().any(|r| {
            r.eq_ignore_ascii_case("SUPER_ADMIN") || r.eq_ignore_ascii_case("PLATFORM_ADMIN")
        });
        let is_school_admin = user.is_school_admin();

        if is_super_admin {
            return NavigationTree {
                top_nav: vec![
                    NavigationItem {
                        name: "Platform Admin".into(),
                        path: "/dashboard/super-admin".into(),
                        icon: "shield".into(),
                        capability: Some("platform:super_admin".into()),
                        badge: None,
                    },
                    NavigationItem {
                        name: "School Tenants".into(),
                        path: "/dashboard/super-admin/schools/new".into(),
                        icon: "school_info".into(),
                        capability: Some("platform:super_admin".into()),
                        badge: None,
                    },
                ],
                users_sidebar: vec![],
                academics_sidebar: vec![],
                school_settings_sidebar: vec![],
                modules_sidebar: vec![],
                platform_sidebar: master.platform_sidebar,
            };
        }

        let can_access = |cap_opt: &Option<String>| -> bool {
            match cap_opt {
                None => !is_super_admin,
                Some(cap) => {
                    if cap == "platform:super_admin" {
                        return is_super_admin;
                    }
                    if is_super_admin {
                        return false;
                    }
                    if is_school_admin {
                        return true;
                    }
                    user.has_capability(cap)
                }
            }
        };

        let filter_section = |sec: &NavigationSection| -> Option<NavigationSection> {
            let filtered_items: Vec<NavigationItem> = sec
                .items
                .iter()
                .filter(|item| can_access(&item.capability))
                .cloned()
                .collect();

            if filtered_items.is_empty() {
                None
            } else {
                Some(NavigationSection {
                    section: sec.section.clone(),
                    items: filtered_items,
                })
            }
        };

        NavigationTree {
            top_nav: master
                .top_nav
                .into_iter()
                .filter(|item| can_access(&item.capability))
                .collect(),
            users_sidebar: master
                .users_sidebar
                .iter()
                .filter_map(filter_section)
                .collect(),
            academics_sidebar: master
                .academics_sidebar
                .iter()
                .filter_map(filter_section)
                .collect(),
            school_settings_sidebar: master
                .school_settings_sidebar
                .iter()
                .filter_map(filter_section)
                .collect(),

            modules_sidebar: master
                .modules_sidebar
                .iter()
                .filter_map(filter_section)
                .collect(),
            platform_sidebar: if is_super_admin {
                master.platform_sidebar
            } else {
                Vec::new()
            },
        }
    }
}
