use crate::{
    modules::admissions::handler::{create_admission, enroll_admission},
    modules::users::{
        admins::add_admin,
        custom_fields::{create_custom_field, delete_custom_field, get_custom_fields},
        handler::{get_users_filter_schema, get_users_list},
        profile::{
            activate_user, bulk_update_users, issue_transfer_certificates, update_user_profile,
        },
        staff_onboarding::{bulk_upload_staff, get_staff_bulk_upload_status, single_add_staff},
        students_bulk::{
            bulk_upload_students, get_student_bulk_upload_status, preview_bulk_students,
            single_add_student,
        },
    },
    state::AppState,
};
use axum::{
    routing::{delete, get, patch, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/list", get(get_users_list))
        .route("/filters/schema", get(get_users_filter_schema))
        .route("/admins", post(add_admin))
        .route("/students/single", post(single_add_student))
        .route("/students/preview-csv", post(preview_bulk_students))
        .route("/students/bulk-upload", post(bulk_upload_students))
        .route(
            "/students/bulk-status/:job_id",
            get(get_student_bulk_upload_status),
        )
        .route(
            "/students/bulk-upload/status/:job_id",
            get(get_student_bulk_upload_status),
        )
        .route("/staff/bulk-upload", post(bulk_upload_staff))
        .route("/staff/bulk", post(bulk_upload_staff))
        .route(
            "/staff/bulk-status/:job_id",
            get(get_staff_bulk_upload_status),
        )
        .route(
            "/staff/bulk-upload/status/:job_id",
            get(get_staff_bulk_upload_status),
        )
        .route("/staff/single", post(single_add_staff))
        .route("/admissions", post(create_admission))
        .route("/admissions/:id/enroll", post(enroll_admission))
        .route(
            "/custom-fields",
            get(get_custom_fields).post(create_custom_field),
        )
        .route("/custom-fields/:id", delete(delete_custom_field))
        .route("/bulk-update", post(bulk_update_users))
        .route(
            "/transfer-certificate/issue",
            post(issue_transfer_certificates),
        )
        .route("/:id", patch(update_user_profile))
        .route("/:id/activate", post(activate_user))
}
