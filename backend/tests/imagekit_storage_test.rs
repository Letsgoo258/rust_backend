use chrono::Utc;
use eravaya::utils::imagekit::{ImageKitClient, ImageKitUploadOptions};

#[test]
fn test_hmac_sha1_signing_and_verification() {
    let raw_url = "https://ik.imagekit.io/tenyf8kjj/school/branding/school_crest_logo.png";
    let private_key = "private_test_secret_key_12345";
    let (signed_url, expires_at) = ImageKitClient::sign_url(raw_url, private_key, 3600);

    assert!(signed_url.contains("ik-t="));
    assert!(signed_url.contains("ik-s="));
    assert!(expires_at > Utc::now().timestamp());

    let is_valid = ImageKitClient::verify_signature(&signed_url, private_key);
    assert!(
        is_valid,
        "Signature should be valid immediately after creation"
    );
}

#[test]
fn test_tampered_signature_rejection() {
    let raw_url = "https://ik.imagekit.io/tenyf8kjj/documents/admission_123.png";
    let private_key = "private_test_secret_key_12345";
    let (signed_url, _) = ImageKitClient::sign_url(raw_url, private_key, 3600);

    // 1. Path tampering: attacker changes document ID in path
    let tampered_path_url = signed_url.replace("admission_123.png", "admission_999.png");
    assert!(
        !ImageKitClient::verify_signature(&tampered_path_url, private_key),
        "Tampered path must fail signature verification"
    );

    // 2. Signature tampering: attacker changes a character in ik-s
    let mut tampered_sig_url = signed_url.clone();
    if let Some(pos) = tampered_sig_url.find("ik-s=") {
        let replace_pos = pos + 5;
        tampered_sig_url.replace_range(replace_pos..replace_pos + 1, "0");
        assert!(
            !ImageKitClient::verify_signature(&tampered_sig_url, private_key),
            "Tampered signature hash must fail verification"
        );
    }

    // 3. Expired timestamp: URL signed with negative expiry
    let expired_timestamp = Utc::now().timestamp() - 500;
    let expired_url = format!("{raw_url}?ik-t={expired_timestamp}&ik-s=fake_sig");
    assert!(
        !ImageKitClient::verify_signature(&expired_url, private_key),
        "Expired URL must be rejected"
    );

    // 4. Wrong private key verification
    assert!(
        !ImageKitClient::verify_signature(&signed_url, "wrong_secret_key_xyz"),
        "Verification with wrong private key must fail"
    );
}

#[test]
fn test_aes_256_gcm_encryption_roundtrip() {
    let original_document = b"CONFIDENTIAL ADMISSION RECORD: Student Aarav Sharma, DOB 2018-05-14";
    let master_key = ImageKitClient::get_encryption_key();

    // Encrypt
    let (ciphertext, iv_hex) = ImageKitClient::encrypt_content(original_document, &master_key)
        .expect("Encryption must succeed");

    assert_ne!(
        ciphertext, original_document,
        "Ciphertext must not match plaintext"
    );
    assert_eq!(iv_hex.len(), 24, "12-byte IV must be 24 hex characters");

    // Decrypt
    let decrypted = ImageKitClient::decrypt_content(&ciphertext, &iv_hex, &master_key)
        .expect("Decryption must succeed");

    assert_eq!(
        decrypted, original_document,
        "Decrypted plaintext must match original payload exactly"
    );
}

#[test]
fn test_tampered_ciphertext_fails_auth_tag() {
    let original = b"Official School Certificate Record";
    let key = ImageKitClient::get_encryption_key();

    let (mut ciphertext, iv_hex) =
        ImageKitClient::encrypt_content(original, &key).expect("Encryption must succeed");

    // Tamper with ciphertext by flipping bits in the first byte
    ciphertext[0] ^= 0xFF;

    let result = ImageKitClient::decrypt_content(&ciphertext, &iv_hex, &key);
    assert!(
        result.is_err(),
        "AES-256-GCM must reject tampered ciphertext with authentication error"
    );
}

#[test]
fn test_sha256_checksum() {
    let payload = b"Eravaya School ERP Digital Asset Integrity";
    let checksum = ImageKitClient::compute_sha256(payload);

    assert_eq!(
        checksum.len(),
        64,
        "SHA-256 hex digest must be 64 characters"
    );
    // Verify determinism
    assert_eq!(checksum, ImageKitClient::compute_sha256(payload));
    // Verify different payload gives different checksum
    assert_ne!(
        checksum,
        ImageKitClient::compute_sha256(b"Different Payload")
    );
}

#[tokio::test]
async fn test_imagekit_upload_and_delete_live() {
    dotenvy::dotenv().ok();
    let priv_key = ImageKitClient::get_private_key();
    if priv_key.trim().is_empty() || priv_key.starts_with("mock_") {
        println!("Skipping live ImageKit test: credentials not configured in environment");
        return;
    }

    // 1x1 transparent PNG bytes
    let sample_png = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=",
    )
    .unwrap();

    let opts = ImageKitUploadOptions {
        file_name: format!("ci_test_{}.png", uuid::Uuid::new_v4()),
        folder: "/ci_tests".to_string(),
        mime_type: Some("image/png".to_string()),
        tags: vec!["ci_test".to_string()],
        use_unique_file_name: true,
        is_private: false,
        encrypt: false,
        retention_days: Some(1),
        school_id: None,
        entity_type: Some("CI_TEST".to_string()),
        entity_id: None,
    };

    // 1. Test Upload
    let upload_res = ImageKitClient::upload_file(sample_png, opts, None)
        .await
        .expect("Live ImageKit upload must succeed");

    assert!(
        !upload_res.file_id.is_empty(),
        "ImageKit must return a file_id"
    );
    assert!(
        upload_res.url.starts_with("https://ik.imagekit.io/"),
        "URL must be an ImageKit CDN URL"
    );
    assert!(
        upload_res.signed_url_24h.contains("ik-s="),
        "Signed URL must contain ik-s"
    );

    // 2. Test Verification of the generated signed URL
    let is_valid = ImageKitClient::verify_signature(&upload_res.signed_url_24h, &priv_key);
    assert!(is_valid, "Generated 24h signed URL must pass verification");

    // 3. Test Delete
    let delete_res = ImageKitClient::delete_file(&upload_res.file_id, None).await;
    assert!(delete_res.is_ok(), "Live ImageKit delete must succeed");
}
