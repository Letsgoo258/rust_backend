
#[derive(Debug, serde::Serialize, sqlx::FromRow, Clone)]
pub struct LookupStudentRow {
    pub id: Uuid,
    pub display_name: String,
    pub username: String,
    pub admission_number: String,
    pub current_class_section_id: Option<Uuid>,
    pub current_class_name: Option<String>,
    pub current_section_name: Option<String>,
}
