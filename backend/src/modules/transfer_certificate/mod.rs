pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

pub use handler::TransferCertificateHandler;
#[allow(unused_imports)]
pub use repository::TransferCertificateRepository;
#[allow(unused_imports)]
pub use service::{format_document_number, TransferCertificateService};

use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/template",
            get(TransferCertificateHandler::get_template)
                .post(TransferCertificateHandler::save_template),
        )
        .route(
            "/template/publish",
            post(TransferCertificateHandler::publish_template),
        )
        .route(
            "/template/request-otp",
            post(TransferCertificateHandler::request_otp),
        )
        .route(
            "/template/verify-otp",
            post(TransferCertificateHandler::verify_otp),
        )
        .route(
            "/template/versions",
            get(TransferCertificateHandler::list_versions),
        )
        .route(
            "/template/rollback",
            post(TransferCertificateHandler::rollback_version),
        )
        .route("/sequences", get(TransferCertificateHandler::get_sequences))
        .route(
            "/sequences/:report_type",
            get(TransferCertificateHandler::get_sequence_detail)
                .put(TransferCertificateHandler::update_sequence),
        )
        .route("/issue", post(TransferCertificateHandler::issue_report))
        .route(
            "/issued",
            get(TransferCertificateHandler::list_issued_reports),
        )
        .route(
            "/issued/:id/cancel",
            post(TransferCertificateHandler::cancel_issued_report),
        )
        .route(
            "/students/issue-tc",
            post(TransferCertificateHandler::issue_transfer_certificates),
        )
        .layer(axum::extract::DefaultBodyLimit::max(20 * 1024 * 1024))
}
