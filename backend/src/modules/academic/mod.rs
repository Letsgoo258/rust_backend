pub mod calendar;
pub mod electives;
pub mod exams;
pub mod handler;
pub mod repository;
pub mod timetable;

#[allow(unused_imports)]
pub use repository::AcademicRepository;
#[allow(unused_imports)]
pub use timetable::start_daily_timetable_worker;
