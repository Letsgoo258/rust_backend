use crate::error::AppError;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const ROTATION_INTERVAL_SECS: u64 = 600; // 10 minutes

static MASTER_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

fn get_master_secret() -> &'static [u8] {
    MASTER_SECRET.get_or_init(|| {
        if let Ok(sec) = std::env::var("DASHBOARD_SIGNING_SECRET") {
            if !sec.trim().is_empty() {
                return sec.into_bytes();
            }
        }
        // Fallback default secret for development/continuity
        b"eravaya_enterprise_dashboard_signing_master_key_2026".to_vec()
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardSignature {
    pub signature: String,
    pub epoch: u64,
    pub expires_in_secs: u64,
}

pub struct UrlSigner;

impl UrlSigner {
    /// Returns the current 10-minute epoch window number
    pub fn current_epoch() -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now / ROTATION_INTERVAL_SECS
    }

    /// Returns seconds remaining in the current 10-minute epoch window
    pub fn current_epoch_remaining_secs() -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let elapsed_in_window = now % ROTATION_INTERVAL_SECS;
        ROTATION_INTERVAL_SECS.saturating_sub(elapsed_in_window)
    }

    /// Derive an epoch-specific 256-bit key from the master secret
    fn derive_epoch_key(epoch: u64) -> Vec<u8> {
        let master = get_master_secret();
        let mut mac = HmacSha256::new_from_slice(master).expect("HMAC can take key of any size");
        mac.update(format!("eravaya:url_epoch:{}", epoch).as_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    /// Compute HMAC signature for given user, school, and epoch
    pub fn compute_signature(user_id: Uuid, school_id: Uuid, epoch: u64) -> String {
        let epoch_key = Self::derive_epoch_key(epoch);
        let mut mac =
            HmacSha256::new_from_slice(&epoch_key).expect("HMAC can take key of any size");
        let payload = format!("{}:{}:{}", user_id, school_id, epoch);
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Generates a signed dashboard session object with current epoch and TTL
    pub fn sign_dashboard_session(user_id: Uuid, school_id: Uuid) -> DashboardSignature {
        let epoch = Self::current_epoch();
        let signature = Self::compute_signature(user_id, school_id, epoch);
        let expires_in_secs = Self::current_epoch_remaining_secs();

        DashboardSignature {
            signature,
            epoch,
            expires_in_secs,
        }
    }

    /// Verifies the signature. Accepts signatures from the current epoch (t) or previous epoch (t-1)
    /// to provide a 10-minute grace period across rotation boundaries.
    pub fn verify(
        user_id: Uuid,
        school_id: Uuid,
        signature: &str,
        epoch: u64,
    ) -> Result<(), AppError> {
        let current = Self::current_epoch();

        // Only allow current epoch (t) and previous epoch (t-1)
        if epoch != current && epoch != current.saturating_sub(1) {
            tracing::warn!(
                user_id = %user_id,
                provided_epoch = epoch,
                current_epoch = current,
                "URL Signature rejected: epoch expired or in future"
            );
            return Err(AppError::Forbidden(
                "URL signature expired or out of bounds. Please refresh session.".into(),
            ));
        }

        let expected_signature = Self::compute_signature(user_id, school_id, epoch);

        // Constant time comparison
        if expected_signature != signature {
            tracing::warn!(
                user_id = %user_id,
                provided_sig = %signature,
                "URL Signature rejected: cryptographic mismatch / tampering detected"
            );
            return Err(AppError::Forbidden(
                "Invalid or tampered URL signature.".into(),
            ));
        }

        Ok(())
    }
}
