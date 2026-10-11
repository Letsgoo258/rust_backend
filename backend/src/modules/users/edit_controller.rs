use crate::error::AppError;
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UserEditPayload {
    // Critical Fields (Require Admin / Ticket)
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub date_of_birth: Option<String>, // Or date type
    pub email: Option<String>,
    pub phone: Option<String>,

    // Standard Fields
    pub gender: Option<String>,
    pub blood_group: Option<String>,
    pub address: Option<String>,
    pub status: Option<String>,
    // Non-negotiable (not included in payload because it can never be changed)
}

#[allow(dead_code)]
pub struct UserEditController;

#[allow(dead_code)]
impl UserEditController {
    /// Validates and processes a single user edit request based on strict role policies
    pub async fn process_single_edit(
        // actor_roles: &[String],
        is_admin: bool,
        payload: &UserEditPayload,
    ) -> Result<(), AppError> {
        // NON-NEGOTIABLE: System ID or Data UUID assigned to the user cannot be changed.
        // (Handled by design since it's not even accepted in the payload and is strictly used for WHERE clauses in the SQL repository)

        // CRITICAL FIELDS POLICY:
        // First name, Last Name, DOB, Email, Phone
        // These can only be changed directly by an admin, otherwise they must go through the Ticket System.
        let has_critical_changes = payload.first_name.is_some()
            || payload.last_name.is_some()
            || payload.date_of_birth.is_some()
            || payload.email.is_some()
            || payload.phone.is_some();

        if has_critical_changes && !is_admin {
            return Err(AppError::Forbidden(
                "Critical identity data (Name, DOB, Email, Phone) can only be changed by a School Admin or via an  Ticket.".into()
            ));
        }

        // Rest can be changed (standard validation applies)
        Ok(())
    }

    /// Validates and processes a bulk user edit request
    pub async fn process_bulk_edits(
        is_admin: bool,
        has_bulk_edit_role_config: bool,
        _payloads: &[UserEditPayload],
    ) -> Result<(), AppError> {
        // BULK CHANGES POLICY:
        // No bulk changes permitted unless explicitly given to school admins
        // or a specific individual role assignee configured for it.
        if !is_admin && !has_bulk_edit_role_config {
            return Err(AppError::Forbidden(
                "Bulk changes are strictly restricted to School Admins and specifically authorized roles.".into()
            ));
        }

        // Additional critical fields check could be applied across the payloads if necessary

        Ok(())
    }
}
