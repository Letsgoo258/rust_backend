use super::dto::{
    CancelIssuedReportPayload, IssueReportPayload, IssueTransferCertificatesRequest,
    ListIssuedReportsQuery, RollbackPayload, SaveTemplatePayload, TemplateQuery,
    UpdateSequencePayload, VerifyOtpPayload,
};
use super::service::TransferCertificateService;
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

pub struct TransferCertificateHandler;

impl TransferCertificateHandler {
    /// GET /template
    pub async fn get_template(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(query): Query<TemplateQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let val = TransferCertificateService::get_template(
            &state.db,
            user.school_id,
            query.template_type.as_deref(),
        )
        .await?;

        Ok((StatusCode::OK, Json(json!({ "template": val }))))
    }

    /// POST /template
    pub async fn save_template(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<SaveTemplatePayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_published = payload.is_published.unwrap_or(false);
        let template_obj = TransferCertificateService::save_template(
            &state.db,
            user.school_id,
            user.id,
            payload,
            is_published,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": "Template saved successfully",
                "template": template_obj
            })),
        ))
    }

    /// POST /template/publish
    pub async fn publish_template(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<SaveTemplatePayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let template_obj = TransferCertificateService::save_template(
            &state.db,
            user.school_id,
            user.id,
            payload,
            true,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": "Template published successfully",
                "template": template_obj
            })),
        ))
    }

    /// POST /template/request-otp
    pub async fn request_otp(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let otp =
            TransferCertificateService::request_otp(&state.db, user.school_id, user.id).await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": "OTP sent to School Admin phone/email",
                "otp_hint": otp
            })),
        ))
    }

    /// POST /template/verify-otp
    pub async fn verify_otp(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<VerifyOtpPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let unlock_token = TransferCertificateService::verify_otp(
            &state.db,
            user.school_id,
            user.id,
            &payload.otp_code,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": "Template layout successfully unlocked for editing",
                "unlock_token": unlock_token
            })),
        ))
    }

    /// GET /template/versions
    pub async fn list_versions(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(query): Query<TemplateQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let versions = TransferCertificateService::list_versions(
            &state.db,
            user.school_id,
            query.template_type.as_deref(),
        )
        .await?;

        Ok((StatusCode::OK, Json(json!({ "versions": versions }))))
    }

    /// POST /template/rollback
    pub async fn rollback_version(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<RollbackPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let updated_obj = TransferCertificateService::rollback_version(
            &state.db,
            user.school_id,
            user.id,
            payload,
        )
        .await?;

        let ver = updated_obj
            .get("version")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": format!("Rolled back to snapshot version {}", ver - 1),
                "template": updated_obj
            })),
        ))
    }

    /// GET /sequences
    pub async fn get_sequences(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let sequences =
            TransferCertificateService::get_sequences(&state.db, user.school_id).await?;

        Ok((StatusCode::OK, Json(json!({ "sequences": sequences }))))
    }

    /// GET /sequences/:report_type
    pub async fn get_sequence_detail(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(report_type): Path<String>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let (seq, current_formatted, next_num, next_formatted) =
            TransferCertificateService::get_sequence_detail(
                &state.db,
                user.school_id,
                &report_type,
            )
            .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "sequence": seq,
                "current_formatted_number": current_formatted,
                "next_number": next_num,
                "next_formatted_number": next_formatted
            })),
        ))
    }

    /// PUT /sequences/:report_type
    pub async fn update_sequence(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(report_type): Path<String>,
        Json(payload): Json<UpdateSequencePayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let (updated, next_num, next_formatted) = TransferCertificateService::update_sequence(
            &state.db,
            user.school_id,
            user.id,
            &report_type,
            payload,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": "Report sequence updated successfully",
                "sequence": updated,
                "next_number": next_num,
                "next_formatted_number": next_formatted
            })),
        ))
    }

    /// POST /issue
    pub async fn issue_report(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<IssueReportPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let issuer_name = user
            .display_name
            .clone()
            .unwrap_or_else(|| user.username.clone());

        let (issued_docs, display_name, current_counter, next_formatted) =
            TransferCertificateService::issue_report(
                &state.db,
                user.school_id,
                user.id,
                &issuer_name,
                payload,
            )
            .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": format!("Successfully issued {} {} document(s)", issued_docs.len(), display_name),
                "issued_documents": issued_docs,
                "new_current_number": current_counter,
                "next_formatted_number": next_formatted
            })),
        ))
    }

    /// GET /issued
    pub async fn list_issued_reports(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(query): Query<ListIssuedReportsQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let (rows, total, page, limit) =
            TransferCertificateService::list_issued_reports(&state.db, user.school_id, query)
                .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "reports": rows,
                "total": total,
                "page": page,
                "limit": limit
            })),
        ))
    }

    /// POST /issued/:id/cancel
    pub async fn cancel_issued_report(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
        Json(payload): Json<CancelIssuedReportPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let cancelled_id = TransferCertificateService::cancel_issued_report(
            &state.db,
            user.school_id,
            user.id,
            id,
            payload,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(json!({
                "message": "Issued report cancelled successfully",
                "id": cancelled_id,
                "status": "CANCELLED"
            })),
        ))
    }

    /// POST /students/issue-tc
    pub async fn issue_transfer_certificates(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<IssueTransferCertificatesRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let auth_user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_authorized = auth_user.is_school_admin()
            || auth_user
                .roles
                .iter()
                .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"))
            || auth_user.has_capability("certificate:issue_tc");

        if !is_authorized {
            return Err(AppError::Forbidden(
                "Permission denied: certificate:issue_tc capability required to issue Transfer Certificates".into(),
            ));
        }

        let is_super_admin = auth_user.roles.contains(&"SUPER_ADMIN".to_string());

        let res = TransferCertificateService::issue_transfer_certificates(
            &state.db,
            auth_user.school_id,
            auth_user.id,
            is_super_admin,
            payload,
        )
        .await?;

        crate::modules::users::repository::UserRepository::invalidate_users_cache(Some(
            auth_user.school_id,
        ));

        Ok((StatusCode::OK, Json(res)))
    }
}
