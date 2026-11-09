use crate::state::AppState;
use axum::Router;

// Re-export all Transfer Certificate domain items for seamless backwards compatibility
#[allow(unused_imports)]
pub use crate::modules::transfer_certificate::{
    dto::*,
    format_document_number,
    handler::TransferCertificateHandler,
    repository::{IssuedReport, ReportSequence, TransferCertificateRepository},
    service::TransferCertificateService,
};

/// Route handler mounting all reports and transfer certificate studio endpoints
pub fn router() -> Router<AppState> {
    crate::modules::transfer_certificate::router()
}
