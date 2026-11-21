use crate::{modules::lms::LmsController, state::AppState};
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks",
            post(LmsController::create_task).get(LmsController::list_tasks),
        )
        .route(
            "/tasks/:id",
            get(LmsController::get_task_detail).put(LmsController::update_task),
        )
        .route("/tasks/:id/publish", post(LmsController::publish_task))
        .route("/tasks/:id/cancel", post(LmsController::cancel_task))
        .route(
            "/tasks/:id/attachments",
            post(LmsController::add_attachment),
        )
        .route(
            "/tasks/:id/attachments/:attachment_id",
            delete(LmsController::delete_attachment),
        )
        .route(
            "/tasks/:id/evaluation/marks",
            get(LmsController::get_evaluation_marks).post(LmsController::save_evaluation_marks),
        )
        .route("/overview-stats", get(LmsController::get_overview_stats))
        .route(
            "/default-attendance",
            get(LmsController::get_default_attendance),
        )
        .route(
            "/attended-classes",
            get(LmsController::get_attended_classes),
        )
        .route("/daily-report", get(LmsController::get_daily_report))
}
