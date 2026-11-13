use crate::{
    modules::academic::handler::{
        assign_students_bulk, bulk_assign_class, create_academic_year, create_bulk_classes,
        create_bulk_subject_types, create_bulk_subjects, delete_subject, delete_subject_type,
        get_academic_teachers, get_academic_years, get_academics_dashboard_config,
        get_class_capacity, get_class_section_history, get_class_section_students, get_my_class,
        get_school_stats, get_subject_types, get_subjects, lookup_admission_numbers,
        update_class_section, update_subject,
    },
    state::AppState,
};
use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/years", post(create_academic_year).get(get_academic_years))
        .route("/classes", post(create_bulk_classes))
        .route("/subjects", post(create_bulk_subjects).get(get_subjects))
        .route("/subjects/:id", put(update_subject).delete(delete_subject))
        .route(
            "/subject-types",
            post(create_bulk_subject_types).get(get_subject_types),
        )
        .route("/subject-types/:id", delete(delete_subject_type))
        .route("/class-capacity", get(get_class_capacity))
        .route("/class-section/update", post(update_class_section))
        .route("/class-section/:id/history", get(get_class_section_history))
        .route(
            "/class-section/:id/students",
            get(get_class_section_students),
        )
        .route(
            "/classes/:id/students",
            get(get_class_section_students),
        )
        .route(
            "/class-section/:id/core-subjects",
            get(crate::modules::academic::exams::ExamHandler::get_section_core_subjects),
        )
        .route("/my-class", get(get_my_class))
        .route("/teachers", get(get_academic_teachers))
        .route("/stats", get(get_school_stats))
        .route("/assign-students-bulk", post(assign_students_bulk))
        .route("/bulk-assign-class", post(bulk_assign_class))
        .route("/lookup-admission-numbers", post(lookup_admission_numbers))
        .route("/dashboard-config", get(get_academics_dashboard_config))
        // Timetable & Substitutions
        .route(
            "/timetable/section/:id",
            get(crate::modules::academic::timetable::TimetableHandler::get_section_timetable),
        )
        .route(
            "/timetable/teacher-slots",
            get(crate::modules::academic::timetable::TimetableHandler::get_teacher_slots),
        )
        .route(
            "/timetable/slots",
            post(crate::modules::academic::timetable::TimetableHandler::batch_upsert_slots),
        )
        .route(
            "/timetable/slots/:id",
            delete(crate::modules::academic::timetable::TimetableHandler::delete_slot),
        )
        .route(
            "/timetable/auto-generate",
            post(crate::modules::academic::timetable::TimetableHandler::auto_generate_preview),
        )
        .route(
            "/timetable/substitutions",
            get(crate::modules::academic::timetable::TimetableHandler::list_substitutions)
                .post(crate::modules::academic::timetable::TimetableHandler::create_substitution),
        )
        .route(
            "/timetable/substitutions/:id",
            delete(crate::modules::academic::timetable::TimetableHandler::cancel_substitution),
        )
        .route(
            "/timetable/substitutions/available-teachers",
            get(crate::modules::academic::timetable::TimetableHandler::get_available_substitute_teachers),
        )
        .route(
            "/timetable/today",
            get(crate::modules::academic::timetable::TimetableHandler::get_today_schedule),
        )
        .route(
            "/timetable/daily/sync",
            post(crate::modules::academic::timetable::TimetableHandler::sync_daily_timetable),
        )
        .route(
            "/timetable/check-clash",
            post(crate::modules::academic::timetable::TimetableHandler::check_clash),
        )
        .route(
            "/timetable/config",
            get(crate::modules::academic::timetable::TimetableHandler::get_timetable_config)
                .post(crate::modules::academic::timetable::TimetableHandler::save_timetable_config),
        )
        // Elective Groups & Student Segregation
        .route(
            "/electives/groups",
            get(crate::modules::academic::electives::ElectiveHandler::list_groups)
                .post(crate::modules::academic::electives::ElectiveHandler::create_group),
        )
        .route(
            "/electives/groups/:id",
            put(crate::modules::academic::electives::ElectiveHandler::update_group)
                .delete(crate::modules::academic::electives::ElectiveHandler::delete_group),
        )
        .route(
            "/electives/enrollments",
            get(crate::modules::academic::electives::ElectiveHandler::get_enrollments),
        )
        .route(
            "/electives/enrollments/bulk",
            post(crate::modules::academic::electives::ElectiveHandler::bulk_assign),
        )
        .route(
            "/electives/matrix",
            get(crate::modules::academic::electives::ElectiveHandler::get_matrix),
        )
        // Academic Calendar & Holidays
        .route(
            "/calendar",
            get(crate::modules::academic::calendar::CalendarHandler::list_calendar)
                .post(crate::modules::academic::calendar::CalendarHandler::create_calendar_entry),
        )
        .route(
            "/calendar/:id",
            delete(crate::modules::academic::calendar::CalendarHandler::delete_calendar_entry),
        )
        .route(
            "/calendar/check",
            get(crate::modules::academic::calendar::CalendarHandler::check_holiday),
        )
        // Exam Events & Timetable Scheduling
        .route(
            "/exam-events",
            get(crate::modules::academic::exams::ExamHandler::list_exam_events)
                .post(crate::modules::academic::exams::ExamHandler::create_exam_event),
        )
        .route(
            "/exam-events/:id",
            get(crate::modules::academic::exams::ExamHandler::get_exam_event_detail),
        )
        .route(
            "/exam-events/:id/classes",
            post(crate::modules::academic::exams::ExamHandler::add_class_schedule),
        )
        .route(
            "/exam-events/:event_id/classes/:class_section_id/timetable",
            post(crate::modules::academic::exams::ExamHandler::save_class_timetable),
        )
        .route(
            "/exam-events/:event_id/classes/:class_section_id/publish-whatsapp",
            post(crate::modules::academic::exams::ExamHandler::publish_class_whatsapp),
        )
        .route(
            "/exam-events/:event_id/classes/:schedule_id",
            delete(crate::modules::academic::exams::ExamHandler::delete_class_schedule),
        )
        // Exams & Assignments Recording Engine
        .route(
            "/exams",
            get(crate::modules::academic::exams::ExamHandler::list_exams)
                .post(crate::modules::academic::exams::ExamHandler::create_exam),
        )
        .route(
            "/exams/:id",
            get(crate::modules::academic::exams::ExamHandler::get_exam_detail),
        )
        .route(
            "/exams/:id/marks",
            post(crate::modules::academic::exams::ExamHandler::save_exam_marks),
        )
        .route(
            "/exams/:id/notify",
            post(crate::modules::academic::exams::ExamHandler::notify_exam),
        )
        .route(
            "/assignments",
            get(crate::modules::academic::exams::ExamHandler::list_assignments)
                .post(crate::modules::academic::exams::ExamHandler::create_assignment),
        )
        .route(
            "/assignments/:id",
            get(crate::modules::academic::exams::ExamHandler::get_assignment_detail),
        )
        .route(
            "/assignments/:id/marks",
            post(crate::modules::academic::exams::ExamHandler::save_assignment_marks),
        )
        .route(
            "/assignments/:id/notify",
            post(crate::modules::academic::exams::ExamHandler::notify_assignment),
        )
        .route(
            "/class-section/:id/scores",
            get(crate::modules::academic::exams::ExamHandler::get_class_section_scores),
        )
}
