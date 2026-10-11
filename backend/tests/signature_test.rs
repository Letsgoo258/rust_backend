use eravaya::utils::url_signer::{UrlSigner, ROTATION_INTERVAL_SECS};
use uuid::Uuid;

#[test]
fn test_signature_generation_and_validation() {
    let user_id = Uuid::new_v4();
    let school_id = Uuid::new_v4();

    let signed_session = UrlSigner::sign_dashboard_session(user_id, school_id);
    assert!(!signed_session.signature.is_empty());
    assert!(signed_session.expires_in_secs <= ROTATION_INTERVAL_SECS);

    // Valid signature with current epoch
    let verify_res = UrlSigner::verify(
        user_id,
        school_id,
        &signed_session.signature,
        signed_session.epoch,
    );
    assert!(
        verify_res.is_ok(),
        "Current epoch signature should be valid"
    );
}

#[test]
fn test_signature_tamper_detection() {
    let user_id = Uuid::new_v4();
    let other_user = Uuid::new_v4();
    let school_id = Uuid::new_v4();

    let signed = UrlSigner::sign_dashboard_session(user_id, school_id);

    // 1. Modified signature payload
    let tampered_sig = format!(
        "{}deadbeef",
        &signed.signature[..signed.signature.len() - 8]
    );
    let verify_tampered = UrlSigner::verify(user_id, school_id, &tampered_sig, signed.epoch);
    assert!(
        verify_tampered.is_err(),
        "Tampered signature string must be rejected"
    );

    // 2. Transferred to another user (impersonation / session hijacking attempt)
    let verify_impersonated =
        UrlSigner::verify(other_user, school_id, &signed.signature, signed.epoch);
    assert!(
        verify_impersonated.is_err(),
        "Signature must not validate for different user ID"
    );
}

#[test]
fn test_10_minute_epoch_grace_period_and_expiration() {
    let user_id = Uuid::new_v4();
    let school_id = Uuid::new_v4();
    let current_epoch = UrlSigner::current_epoch();

    // Signature from previous epoch (t-1) - Should be accepted (10-min grace period)
    let prev_epoch = current_epoch.saturating_sub(1);
    let prev_sig = UrlSigner::compute_signature(user_id, school_id, prev_epoch);
    let prev_res = UrlSigner::verify(user_id, school_id, &prev_sig, prev_epoch);
    assert!(
        prev_res.is_ok(),
        "Previous epoch signature should pass grace period verification"
    );

    // Signature from older epoch (t-2) - Must be rejected (expired continuity)
    if current_epoch >= 2 {
        let expired_epoch = current_epoch - 2;
        let expired_sig = UrlSigner::compute_signature(user_id, school_id, expired_epoch);
        let expired_res = UrlSigner::verify(user_id, school_id, &expired_sig, expired_epoch);
        assert!(
            expired_res.is_err(),
            "Epoch older than 10-minute grace window must be rejected"
        );
    }
}
