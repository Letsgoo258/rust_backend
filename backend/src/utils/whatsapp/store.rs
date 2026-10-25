use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use rand::RngCore;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use wacore::appstate::hash::HashState;
use wacore::store::error::Result as WaResult;
use wacore::store::traits::{
    AppStateSyncKey, AppSyncStore, DeviceListRecord, DeviceStore, LidPnMappingEntry,
    MsgSecretEntry, MsgSecretStore, ProtocolStore, SignalStore, TcTokenEntry,
};
use wacore::store::{Device, DevicePropsOverride, InMemoryBackend};
use wacore_appstate::processor::AppStateMutationMAC;

/// Key derivation helper: hashes input secret into 32-byte key for AES-256-GCM.
pub fn derive_encryption_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypts session bytes with AES-256-GCM.
/// Output format: [12-byte nonce][ciphertext + tag]
pub fn encrypt_session_bytes(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow!("Failed to initialize AES-256-GCM: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow!("Encryption error: {e}"))?;

    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypts session bytes encrypted with AES-256-GCM.
pub fn decrypt_session_bytes(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if encrypted.len() < 12 {
        return Err(anyhow!("Encrypted payload too short (missing nonce)"));
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow!("Failed to initialize AES-256-GCM: {e}"))?;

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed (invalid key or corrupted data): {e}"))?;

    Ok(plaintext)
}

/// PostgreSQL-backed, AES-256-GCM encrypted WhatsApp store.
/// Satisfies `Backend` (`SignalStore + AppSyncStore + ProtocolStore + MsgSecretStore + DeviceStore`).
pub struct PostgresStore {
    inner: InMemoryBackend,
    pool: PgPool,
    encryption_key: [u8; 32],
}

impl PostgresStore {
    pub async fn init(pool: PgPool, secret: &str) -> Result<Arc<Self>> {
        let encryption_key = derive_encryption_key(secret);
        let inner = InMemoryBackend::new();

        info!("Initializing PostgreSQL-backed WhatsApp session store...");

        // Load all saved state from whatsapp_session_store
        let rows =
            sqlx::query_as::<_, (String, Vec<u8>)>("SELECT key, value FROM whatsapp_session_store")
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

        let mut restored_count = 0usize;
        for (k, encrypted) in rows {
            if let Ok(raw) = decrypt_session_bytes(&encrypted, &encryption_key) {
                if k == "primary_device" {
                    if let Ok(mut device) = serde_json::from_slice::<Device>(&raw) {
                        if device.pn.is_some() || device.account.is_some() {
                            let app_version =
                                whatsapp_rust::waproto::whatsapp::device_props::AppVersion {
                                    primary: Some(2),
                                    secondary: Some(3000),
                                    tertiary: Some(1051602143),
                                    ..Default::default()
                                };
                            let props_override = DevicePropsOverride::new()
                                .with_os("macOS")
                                .with_platform_type(whatsapp_rust::waproto::whatsapp::device_props::PlatformType::CHROME)
                                .with_version(app_version);
                            device.set_device_props(props_override);
                            device.client_profile = wacore::client_profile::ClientProfile::web();
                            let _ = inner.save(&device).await;
                            restored_count += 1;
                        } else {
                            tracing::warn!("Discarding unlinked primary_device snapshot to allow fresh QR pairing.");
                            let _ = sqlx::query(
                                "DELETE FROM whatsapp_session_store WHERE key = 'primary_device'",
                            )
                            .execute(&pool)
                            .await;
                        }
                    }
                } else if let Some(addr) = k.strip_prefix("identity:") {
                    if raw.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&raw);
                        let _ = inner.put_identity(addr, arr).await;
                        restored_count += 1;
                    }
                } else if let Some(addr) = k.strip_prefix("session:") {
                    let _ = inner.put_session(addr, &raw).await;
                    restored_count += 1;
                } else if let Some(id_str) = k.strip_prefix("prekey:") {
                    if let Ok(id) = id_str.parse::<u32>() {
                        let _ = inner.store_prekey(id, &raw, true).await;
                        restored_count += 1;
                    }
                } else if let Some(id_str) = k.strip_prefix("signed_prekey:") {
                    if let Ok(id) = id_str.parse::<u32>() {
                        let _ = inner.store_signed_prekey(id, &raw).await;
                        restored_count += 1;
                    }
                } else if let Some(name) = k.strip_prefix("sender_key:") {
                    let _ = inner.put_sender_key(name, &raw).await;
                    restored_count += 1;
                }
            }
        }

        info!(
            restored_entries = restored_count,
            "WhatsApp encrypted session store restored successfully from PostgreSQL"
        );

        Ok(Arc::new(Self {
            inner,
            pool,
            encryption_key,
        }))
    }

    async fn db_set(&self, key: &str, raw_value: &[u8]) -> WaResult<()> {
        let encrypted = encrypt_session_bytes(raw_value, &self.encryption_key)
            .map_err(|e| wacore::store::error::StoreError::Io(std::io::Error::other(e)))?;

        sqlx::query(
            r#"
            INSERT INTO whatsapp_session_store (key, value, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (key) DO UPDATE
            SET value = EXCLUDED.value, updated_at = NOW()
            "#,
        )
        .bind(key)
        .bind(encrypted)
        .execute(&self.pool)
        .await
        .map_err(|e| wacore::store::error::StoreError::Io(std::io::Error::other(e)))?;

        Ok(())
    }

    async fn db_delete(&self, key: &str) -> WaResult<()> {
        sqlx::query("DELETE FROM whatsapp_session_store WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| wacore::store::error::StoreError::Io(std::io::Error::other(e)))?;

        Ok(())
    }

    pub async fn backup_snapshot(&self) -> Result<()> {
        if let Some(device) = self.inner.load().await.map_err(|e| anyhow!("{e}"))? {
            if device.pn.is_some() || device.account.is_some() {
                let bytes = serde_json::to_vec(&device)?;
                self.db_set("primary_device", &bytes)
                    .await
                    .map_err(|e| anyhow!("{e}"))?;
                info!("WhatsApp device snapshot backed up to PostgreSQL successfully");
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SignalStore
// ---------------------------------------------------------------------------
#[async_trait]
impl SignalStore for PostgresStore {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> WaResult<()> {
        self.inner.put_identity(address, key).await?;
        self.db_set(&format!("identity:{address}"), &key).await?;
        Ok(())
    }

    async fn load_identity(&self, address: &str) -> WaResult<Option<[u8; 32]>> {
        self.inner.load_identity(address).await
    }

    async fn delete_identity(&self, address: &str) -> WaResult<()> {
        self.inner.delete_identity(address).await?;
        self.db_delete(&format!("identity:{address}")).await?;
        Ok(())
    }

    async fn get_session(&self, address: &str) -> WaResult<Option<Bytes>> {
        self.inner.get_session(address).await
    }

    async fn put_session(&self, address: &str, session: &[u8]) -> WaResult<()> {
        self.inner.put_session(address, session).await?;
        self.db_set(&format!("session:{address}"), session).await?;
        Ok(())
    }

    async fn put_sessions_batch(&self, sessions: &[(Arc<str>, Bytes)]) -> WaResult<()> {
        self.inner.put_sessions_batch(sessions).await?;
        for (addr, session) in sessions {
            let _ = self.db_set(&format!("session:{addr}"), session).await;
        }
        Ok(())
    }

    async fn has_session(&self, address: &str) -> WaResult<bool> {
        self.inner.has_session(address).await
    }

    async fn has_signal_state_for_user(&self, user: &str) -> WaResult<bool> {
        self.inner.has_signal_state_for_user(user).await
    }

    async fn delete_session(&self, address: &str) -> WaResult<()> {
        self.inner.delete_session(address).await?;
        self.db_delete(&format!("session:{address}")).await?;
        Ok(())
    }

    async fn store_prekey(&self, id: u32, record: &[u8], uploaded: bool) -> WaResult<()> {
        self.inner.store_prekey(id, record, uploaded).await?;
        self.db_set(&format!("prekey:{id}"), record).await?;
        Ok(())
    }

    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> WaResult<()> {
        self.inner.mark_prekeys_uploaded(ids).await
    }

    async fn load_prekey(&self, id: u32) -> WaResult<Option<Bytes>> {
        self.inner.load_prekey(id).await
    }

    async fn remove_prekey(&self, id: u32) -> WaResult<()> {
        self.inner.remove_prekey(id).await?;
        self.db_delete(&format!("prekey:{id}")).await?;
        Ok(())
    }

    async fn get_max_prekey_id(&self) -> WaResult<u32> {
        self.inner.get_max_prekey_id().await
    }

    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> WaResult<()> {
        self.inner.store_signed_prekey(id, record).await?;
        self.db_set(&format!("signed_prekey:{id}"), record).await?;
        Ok(())
    }

    async fn load_signed_prekey(&self, id: u32) -> WaResult<Option<Vec<u8>>> {
        self.inner.load_signed_prekey(id).await
    }

    async fn load_all_signed_prekeys(&self) -> WaResult<Vec<(u32, Vec<u8>)>> {
        self.inner.load_all_signed_prekeys().await
    }

    async fn remove_signed_prekey(&self, id: u32) -> WaResult<()> {
        self.inner.remove_signed_prekey(id).await?;
        self.db_delete(&format!("signed_prekey:{id}")).await?;
        Ok(())
    }

    async fn put_sender_key(&self, address: &str, record: &[u8]) -> WaResult<()> {
        self.inner.put_sender_key(address, record).await?;
        self.db_set(&format!("sender_key:{address}"), record)
            .await?;
        Ok(())
    }

    async fn get_sender_key(&self, address: &str) -> WaResult<Option<Vec<u8>>> {
        self.inner.get_sender_key(address).await
    }

    async fn delete_sender_key(&self, address: &str) -> WaResult<()> {
        self.inner.delete_sender_key(address).await?;
        self.db_delete(&format!("sender_key:{address}")).await?;
        Ok(())
    }

    async fn put_sender_keys_batch(&self, keys: &[(Arc<str>, Bytes)]) -> WaResult<()> {
        self.inner.put_sender_keys_batch(keys).await?;
        for (name, key) in keys {
            let _ = self.db_set(&format!("sender_key:{name}"), key).await;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AppSyncStore
// ---------------------------------------------------------------------------
#[async_trait]
impl AppSyncStore for PostgresStore {
    async fn get_sync_key(&self, key_id: &[u8]) -> WaResult<Option<AppStateSyncKey>> {
        self.inner.get_sync_key(key_id).await
    }

    async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> WaResult<()> {
        self.inner.set_sync_key(key_id, key).await
    }

    async fn get_version(&self, name: &str) -> WaResult<HashState> {
        self.inner.get_version(name).await
    }

    async fn set_version(&self, name: &str, state: HashState) -> WaResult<()> {
        self.inner.set_version(name, state).await
    }

    async fn put_mutation_macs(
        &self,
        name: &str,
        version: u64,
        mutations: &[AppStateMutationMAC],
    ) -> WaResult<()> {
        self.inner.put_mutation_macs(name, version, mutations).await
    }

    async fn get_mutation_mac(&self, name: &str, index_mac: &[u8]) -> WaResult<Option<Vec<u8>>> {
        self.inner.get_mutation_mac(name, index_mac).await
    }

    async fn get_mutation_macs(
        &self,
        name: &str,
        index_macs: &[[u8; 32]],
    ) -> WaResult<HashMap<[u8; 32], Vec<u8>>> {
        self.inner.get_mutation_macs(name, index_macs).await
    }

    async fn delete_mutation_macs(&self, name: &str, index_macs: &[Vec<u8>]) -> WaResult<()> {
        self.inner.delete_mutation_macs(name, index_macs).await
    }

    async fn clear_mutation_macs(&self, name: &str) -> WaResult<()> {
        self.inner.clear_mutation_macs(name).await
    }

    async fn get_latest_sync_key_id(&self) -> WaResult<Option<Vec<u8>>> {
        self.inner.get_latest_sync_key_id().await
    }
}

// ---------------------------------------------------------------------------
// ProtocolStore
// ---------------------------------------------------------------------------
#[async_trait]
impl ProtocolStore for PostgresStore {
    async fn get_sender_key_devices(&self, group_jid: &str) -> WaResult<Vec<(String, bool)>> {
        self.inner.get_sender_key_devices(group_jid).await
    }

    async fn set_sender_key_status(
        &self,
        group_jid: &str,
        entries: &[(&str, bool)],
    ) -> WaResult<()> {
        self.inner.set_sender_key_status(group_jid, entries).await
    }

    async fn clear_sender_key_devices(&self, group_jid: &str) -> WaResult<()> {
        self.inner.clear_sender_key_devices(group_jid).await
    }

    async fn delete_sender_key_device_rows(&self, device_jids: &[&str]) -> WaResult<()> {
        self.inner.delete_sender_key_device_rows(device_jids).await
    }

    async fn clear_all_sender_key_devices(&self) -> WaResult<()> {
        self.inner.clear_all_sender_key_devices().await
    }

    async fn get_lid_mapping(&self, lid: &str) -> WaResult<Option<LidPnMappingEntry>> {
        self.inner.get_lid_mapping(lid).await
    }

    async fn get_pn_mapping(&self, phone: &str) -> WaResult<Option<LidPnMappingEntry>> {
        self.inner.get_pn_mapping(phone).await
    }

    async fn put_lid_mapping(&self, entry: &LidPnMappingEntry) -> WaResult<()> {
        self.inner.put_lid_mapping(entry).await
    }

    async fn put_lid_mappings(&self, entries: &[LidPnMappingEntry]) -> WaResult<()> {
        self.inner.put_lid_mappings(entries).await
    }

    async fn get_all_lid_mappings(&self) -> WaResult<Vec<LidPnMappingEntry>> {
        self.inner.get_all_lid_mappings().await
    }

    async fn save_base_key(
        &self,
        address: &str,
        message_id: &str,
        base_key: &[u8],
    ) -> WaResult<()> {
        self.inner
            .save_base_key(address, message_id, base_key)
            .await
    }

    async fn has_same_base_key(
        &self,
        address: &str,
        message_id: &str,
        current_base_key: &[u8],
    ) -> WaResult<bool> {
        self.inner
            .has_same_base_key(address, message_id, current_base_key)
            .await
    }

    async fn delete_base_key(&self, address: &str, message_id: &str) -> WaResult<()> {
        self.inner.delete_base_key(address, message_id).await
    }

    async fn update_device_list(&self, record: DeviceListRecord) -> WaResult<()> {
        self.inner.update_device_list(record).await
    }

    async fn update_device_lists(&self, records: Vec<DeviceListRecord>) -> WaResult<()> {
        self.inner.update_device_lists(records).await
    }

    async fn get_devices(&self, user: &str) -> WaResult<Option<DeviceListRecord>> {
        self.inner.get_devices(user).await
    }

    async fn delete_devices(&self, user: &str) -> WaResult<()> {
        self.inner.delete_devices(user).await
    }

    async fn get_group_metadata(&self, group_jid: &str) -> WaResult<Option<Vec<u8>>> {
        self.inner.get_group_metadata(group_jid).await
    }

    async fn put_group_metadata(&self, group_jid: &str, blob: &[u8]) -> WaResult<()> {
        self.inner.put_group_metadata(group_jid, blob).await
    }

    async fn delete_group_metadata(&self, group_jid: &str) -> WaResult<()> {
        self.inner.delete_group_metadata(group_jid).await
    }

    async fn get_tc_token(&self, jid: &str) -> WaResult<Option<TcTokenEntry>> {
        self.inner.get_tc_token(jid).await
    }

    async fn put_tc_token(&self, jid: &str, entry: &TcTokenEntry) -> WaResult<()> {
        self.inner.put_tc_token(jid, entry).await
    }

    async fn delete_tc_token(&self, jid: &str) -> WaResult<()> {
        self.inner.delete_tc_token(jid).await
    }

    async fn get_all_tc_token_jids(&self) -> WaResult<Vec<String>> {
        self.inner.get_all_tc_token_jids().await
    }

    async fn delete_expired_tc_tokens(
        &self,
        token_cutoff: i64,
        sender_cutoff: i64,
    ) -> WaResult<u32> {
        self.inner
            .delete_expired_tc_tokens(token_cutoff, sender_cutoff)
            .await
    }

    async fn store_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
        payload: &[u8],
    ) -> WaResult<()> {
        self.inner
            .store_sent_message(chat_jid, message_id, payload)
            .await
    }

    async fn take_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
    ) -> WaResult<Option<Vec<u8>>> {
        self.inner.take_sent_message(chat_jid, message_id).await
    }

    async fn delete_expired_sent_messages(&self, cutoff_timestamp: i64) -> WaResult<u32> {
        self.inner
            .delete_expired_sent_messages(cutoff_timestamp)
            .await
    }

    async fn store_pending_inbound(
        &self,
        chat: &str,
        sender: &str,
        id: &str,
        message: &[u8],
    ) -> WaResult<()> {
        self.inner
            .store_pending_inbound(chat, sender, id, message)
            .await
    }

    async fn get_pending_inbound(
        &self,
        chat: &str,
        sender: &str,
        id: &str,
    ) -> WaResult<Option<Vec<u8>>> {
        self.inner.get_pending_inbound(chat, sender, id).await
    }

    async fn delete_pending_inbound(&self, chat: &str, sender: &str, id: &str) -> WaResult<()> {
        self.inner.delete_pending_inbound(chat, sender, id).await
    }

    async fn delete_expired_pending_inbound(&self, cutoff_timestamp: i64) -> WaResult<u32> {
        self.inner
            .delete_expired_pending_inbound(cutoff_timestamp)
            .await
    }
}

// ---------------------------------------------------------------------------
// DeviceStore
// ---------------------------------------------------------------------------
#[async_trait]
impl DeviceStore for PostgresStore {
    async fn save(&self, device: &Device) -> WaResult<()> {
        self.inner.save(device).await?;
        if device.pn.is_some() || device.account.is_some() {
            if let Ok(bytes) = serde_json::to_vec(device) {
                let _ = self.db_set("primary_device", &bytes).await;
            }
        }
        Ok(())
    }

    async fn load(&self) -> WaResult<Option<Device>> {
        self.inner.load().await
    }

    async fn exists(&self) -> WaResult<bool> {
        self.inner.exists().await
    }

    async fn create(&self) -> WaResult<i32> {
        self.inner.create().await
    }

    async fn snapshot_db(&self, _name: &str, _extra_content: Option<&[u8]>) -> WaResult<()> {
        let _ = self.backup_snapshot().await;
        Ok(())
    }

    async fn resource_report(&self) -> wacore::stats::StorageResourceReport {
        self.inner.resource_report().await
    }
}

// ---------------------------------------------------------------------------
// MsgSecretStore
// ---------------------------------------------------------------------------
#[async_trait]
impl MsgSecretStore for PostgresStore {
    async fn put_msg_secrets(&self, entries: Vec<MsgSecretEntry>) -> WaResult<usize> {
        self.inner.put_msg_secrets(entries).await
    }

    async fn get_msg_secret(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> WaResult<Option<Vec<u8>>> {
        self.inner.get_msg_secret(chat, sender, msg_id).await
    }

    async fn delete_expired_msg_secrets(&self, cutoff_timestamp: i64) -> WaResult<u32> {
        self.inner
            .delete_expired_msg_secrets(cutoff_timestamp)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let secret = "super-secret-key-12345";
        let key = derive_encryption_key(secret);
        let sample_data = b"WhatsApp protocol session data binary payload";

        let encrypted = encrypt_session_bytes(sample_data, &key).expect("Encryption failed");
        assert_ne!(&encrypted[12..], sample_data);

        let decrypted = decrypt_session_bytes(&encrypted, &key).expect("Decryption failed");
        assert_eq!(decrypted, sample_data);
    }

    #[test]
    fn test_decryption_with_wrong_key_fails() {
        let key1 = derive_encryption_key("key-one");
        let key2 = derive_encryption_key("key-two");
        let sample_data = b"Confidential cryptographic keys";

        let encrypted = encrypt_session_bytes(sample_data, &key1).unwrap();
        let result = decrypt_session_bytes(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_nonces_produce_different_ciphertexts() {
        let key = derive_encryption_key("same-secret");
        let payload = b"identities, prekeys, and sessions";

        let enc1 = encrypt_session_bytes(payload, &key).unwrap();
        let enc2 = encrypt_session_bytes(payload, &key).unwrap();

        // Nonce and ciphertext must differ due to random 12-byte IV
        assert_ne!(enc1, enc2);

        // Both must decrypt to the original payload
        assert_eq!(decrypt_session_bytes(&enc1, &key).unwrap(), payload);
        assert_eq!(decrypt_session_bytes(&enc2, &key).unwrap(), payload);
    }

    #[test]
    fn test_corrupted_ciphertext_fails_auth() {
        let key = derive_encryption_key("auth-check");
        let payload = b"tamper proof data";

        let mut enc = encrypt_session_bytes(payload, &key).unwrap();
        // Tamper with ciphertext byte
        let last = enc.len() - 1;
        enc[last] ^= 0xFF;

        let res = decrypt_session_bytes(&enc, &key);
        assert!(res.is_err());
    }

    #[test]
    fn test_truncated_ciphertext_fails() {
        let key = derive_encryption_key("length-check");
        let truncated = vec![0u8; 10]; // less than 12-byte nonce
        assert!(decrypt_session_bytes(&truncated, &key).is_err());
    }
}
