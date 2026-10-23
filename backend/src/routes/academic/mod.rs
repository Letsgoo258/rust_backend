use crate::{
    modules::academic::handler::{
        assign_students_bulk, bulk_assign_class, create_academic_year, create_bulk_classes,
        create_bulk_subject_types, create_bulk_subjects, delete_subject, delete_subject_type,
        get_academics_dashboard_config, get_class_capacity, get_class_section_history,
        get_school_stats, get_subject_types, get_subjects, lookup_admission_numbers,
        update_class_section,
    },
    state::AppState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/years", post(create_academic_year))
        .route("/classes", post(create_bulk_classes))
        .route("/subjects", post(create_bulk_subjects).get(get_subjects))
        .route("/subjects/:id", delete(delete_subject))
        .route(
            "/subject-types",
            post(create_bulk_subject_types).get(get_subject_types),
        )
        .route("/subject-types/:id", delete(delete_subject_type))
        .route("/class-capacity", get(get_class_capacity))
        .route("/class-section/update", post(update_class_section))
        .route("/class-section/:id/history", get(get_class_section_history))
        .route("/stats", get(get_school_stats))
        .route("/assign-students-bulk", post(assign_students_bulk))
        .route("/lookup-admission-numbers", post(lookup_admission_numbers))
        .route("/bulk-assign-class", post(bulk_assign_class))
        .route("/dashboard-config", get(get_academics_dashboard_config))
}
