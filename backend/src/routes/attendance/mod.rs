use crate::{modules::attendance::AttendanceHandler, state::AppState};
use axum::{
    routing::{get, post, put},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/record", post(AttendanceHandler::record_attendance))
        .route(
            "/roster/:class_section_id",
            get(AttendanceHandler::get_class_roster),
        )
        .route(
            "/today-slots",
            get(AttendanceHandler::get_today_teacher_slots),
        )
        .route("/bulk-edit", post(AttendanceHandler::bulk_edit_attendance))
        .route("/records/:id", put(AttendanceHandler::update_record))
        .route("/sessions", get(AttendanceHandler::list_sessions))
        .route(
            "/sessions/:id",
            get(AttendanceHandler::get_session_detail).delete(AttendanceHandler::delete_session),
        )
        .route("/register", get(AttendanceHandler::get_register))
        .route("/stats/today", get(AttendanceHandler::get_today_stats))
        .route("/today-stats", get(AttendanceHandler::get_today_stats))
        .route("/collisions", get(AttendanceHandler::list_collisions))
        .route(
            "/collisions/raise",
            post(AttendanceHandler::raise_collision_ticket),
        )
        .route(
            "/collisions/:id/resolve",
            post(AttendanceHandler::resolve_collision),
        )
        .route("/heatmap", get(AttendanceHandler::get_weekly_heatmap))
        .route(
            "/weekly-heatmap",
            get(AttendanceHandler::get_weekly_heatmap),
        )
        .route(
            "/config",
            get(AttendanceHandler::get_config).put(AttendanceHandler::update_config),
        )
}
