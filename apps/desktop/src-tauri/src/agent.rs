use std::env;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::cleanliness::CleanlinessSnapshot;
use crate::command_processor::AgentProposalSubmission;
use crate::smart_cache_crypto::SmartCacheEncryptionMetadata;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SERVER_BASE_URL_ENV: &str = "MOUSEKEEPER_SERVER_BASE_URL";
const KEYRING_SERVICE: &str = "com.mousekeeper.desktop";
const KEYRING_ACCOUNT: &str = "device-token";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AgentConnectionStatus {
    pub state: AgentConnectionState,
    pub server_base_url: Option<String>,
    pub device_id: Option<String>,
    pub last_error_code: Option<AgentErrorCode>,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentConnectionState {
    Unconfigured,
    Offline,
    Connecting,
    Online,
    Revoked,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentCommand {
    pub command_id: String,
    pub command_type: String,
    pub room_id: String,
    pub status: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PairingSession {
    pub session_id: String,
    pub desktop_nonce: String,
    pub code: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PairingStatus {
    pub status: String,
    pub device_id: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HeartbeatResult {
    pub device_id: String,
    pub presence: String,
    pub ttl_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SyncEvent {
    pub event_id: String,
    pub event_type: String,
    pub schema_version: u64,
    pub correlation_id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub device_id: Option<String>,
    pub room_id: Option<String>,
    pub sequence: u64,
    pub occurred_at: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentRoomSync {
    pub room_id: String,
    pub root_id: String,
    pub name: String,
    pub created: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatSession {
    pub session_id: String,
    pub room_id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_preview: String,
    pub unread_count: u64,
    pub pending_action_count: u64,
    pub last_read_message_id: Option<String>,
    pub read_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatMessage {
    pub message_id: String,
    pub room_id: String,
    pub session_id: Option<String>,
    pub sender_type: String,
    pub message_type: String,
    pub content: String,
    pub structured_payload: Value,
    pub command_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatSendResult {
    pub message: AgentChatMessage,
    pub assistant: Option<AgentChatMessage>,
    pub ai_status: String,
    pub ai: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatQuickPrompt {
    pub id: String,
    pub label: String,
    pub prompt: String,
    pub category: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatQuickHistoryItem {
    pub message_id: String,
    pub session_id: String,
    pub session_title: String,
    pub sender_type: String,
    pub message_type: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatQuickSuggestion {
    pub message_id: String,
    pub session_id: String,
    pub session_title: String,
    pub message_type: String,
    pub content: String,
    pub draft_id: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatQuickView {
    pub prompts: Vec<AgentChatQuickPrompt>,
    pub sessions: Vec<AgentChatSession>,
    pub history: Vec<AgentChatQuickHistoryItem>,
    pub pending_suggestions: Vec<AgentChatQuickSuggestion>,
    pub unread_count: u64,
    pub pending_action_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentChatQuickCleanupResult {
    pub session: AgentChatSession,
    pub message: AgentChatMessage,
    pub assistant: Option<AgentChatMessage>,
    pub ai_status: String,
    pub ai: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentProposalItemRecord {
    pub item_order: i64,
    pub action_type: String,
    pub source_relative_path: Option<String>,
    pub destination_relative_path: Option<String>,
    pub reason_code: String,
    pub precondition: Value,
    pub conflict_state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentPendingDecision {
    pub decision_id: String,
    pub proposal_id: String,
    pub room_id: String,
    pub items: Vec<AgentProposalItemRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentExecution {
    pub execution_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentFileBrowseRequest {
    pub request_id: String,
    pub room_id: String,
    pub relative_directory: String,
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub extensions: Vec<String>,
    pub limit: usize,
    pub search_scope: AgentFileSearchScope,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentFileSearchScope {
    #[default]
    CurrentDirectory,
    ManagedRoot,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileBrowseResult {
    pub entries: Vec<AgentFileBrowseEntry>,
    pub next_cursor: Option<String>,
    pub desktop_generation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileBrowseEntry {
    pub name: String,
    pub relative_path: String,
    #[serde(rename = "type")]
    pub entry_type: AgentFileBrowseEntryType,
    pub size_bytes: Option<u64>,
    pub modified_at: String,
    pub file_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentFileBrowseEntryType {
    File,
    Directory,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentFileBrowseFailureCode {
    DeviceOffline,
    TimedOut,
    CursorInvalidated,
    OutsideManagedRoot,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentFileTransfer {
    pub transfer_id: String,
    pub room_id: String,
    pub source_relative_path: String,
    pub status: String,
    pub expires_at: String,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub failure_code: Option<AgentFileTransferFailureCode>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileTransferSourceVersion {
    pub file_id: String,
    pub size_bytes: u64,
    pub modified_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentFileTransferUploadTarget {
    pub transfer_id: String,
    pub upload_url: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentFileTransferFailureCode {
    SourceNotFound,
    SourceChanged,
    OutsideManagedRoot,
    SizeLimitExceeded,
    ChecksumMismatch,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSmartCachePolicy {
    pub room_id: String,
    pub enabled: bool,
    pub quota_bytes: u64,
    pub max_file_bytes: u64,
    pub excluded_patterns: Vec<String>,
    pub pinned_patterns: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSmartCacheCandidate {
    pub source_relative_path: String,
    pub source_version: Value,
    pub source_version_hash: String,
    pub size_bytes: u64,
    pub usage_score: i64,
    pub manual_pin: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSmartCacheReservation {
    pub reservation_id: String,
    pub status: String,
    pub source_relative_path: String,
    pub source_version_hash: String,
    pub size_bytes: u64,
    pub upload_url: Option<String>,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSmartCacheCandidateBatchResult {
    pub batch_id: String,
    pub approved: Vec<AgentSmartCacheReservation>,
    pub rejected_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCachedFile {
    pub cached_file_id: String,
    pub source_relative_path: String,
    pub source_version_hash: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub freshness_status: String,
    pub encryption_metadata: Option<SmartCacheEncryptionMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSmartCacheStaleResult {
    pub room_id: String,
    pub source_relative_path: Option<String>,
    pub reason: String,
    pub stale_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoomSnapshot {
    pub snapshot_id: String,
    pub room_id: String,
    pub formula_version: String,
    pub score: u8,
    pub metrics: Value,
    pub calculated_at: String,
}

/// Connection parameters for the realtime Socket.IO client. Intentionally not `Serialize`: the
/// device token must never leave the process through a Tauri command or a serialized log line.
pub struct RealtimeCredentials {
    pub base_url: String,
    pub device_token: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentErrorCode {
    Unconfigured,
    ValidationFailed,
    TransportUnavailable,
    Unauthenticated,
    Forbidden,
    InvalidResponse,
    CredentialStoreUnavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AgentError {
    pub code: AgentErrorCode,
    pub message: String,
    server_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct DeviceCredential {
    server_base_url: String,
    device_id: String,
    device_token: String,
}

trait CredentialStore: Send + Sync {
    fn load(&self) -> Result<Option<DeviceCredential>, AgentError>;
    fn save(&self, credential: &DeviceCredential) -> Result<(), AgentError>;
    fn delete(&self) -> Result<(), AgentError>;
}

struct KeyringCredentialStore;

impl KeyringCredentialStore {
    fn entry(&self) -> Result<keyring::Entry, AgentError> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|_| credential_error("cannot access the operating system credential store"))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn load(&self) -> Result<Option<DeviceCredential>, AgentError> {
        let entry = self.entry()?;
        let serialized = match entry.get_password() {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(_) => return Err(credential_error("cannot read the saved device credential")),
        };
        serde_json::from_str(&serialized)
            .map(Some)
            .map_err(|_| credential_error("the saved device credential is invalid"))
    }

    fn save(&self, credential: &DeviceCredential) -> Result<(), AgentError> {
        let serialized = serde_json::to_string(credential)
            .map_err(|_| credential_error("cannot encode the device credential"))?;
        self.entry()?
            .set_password(&serialized)
            .map_err(|_| credential_error("cannot save the device credential"))
    }

    fn delete(&self) -> Result<(), AgentError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(credential_error(
                "cannot delete the saved device credential",
            )),
        }
    }
}

struct AgentState {
    server_base_url: Option<String>,
    credential: Option<DeviceCredential>,
    state: AgentConnectionState,
    last_error: Option<AgentError>,
}

pub struct AgentRuntime {
    http: Client,
    credentials: Arc<dyn CredentialStore>,
    state: Mutex<AgentState>,
    room_sync_lock: tokio::sync::Mutex<()>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        let configured_url = env::var(SERVER_BASE_URL_ENV).ok();
        Self::new(configured_url.as_deref(), Arc::new(KeyringCredentialStore))
    }
}

impl AgentRuntime {
    fn new(server_base_url: Option<&str>, credentials: Arc<dyn CredentialStore>) -> Self {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client configuration is static and valid");

        let (server_base_url, credential, state, last_error) = match server_base_url {
            None | Some("") => (
                None,
                None,
                AgentConnectionState::Unconfigured,
                Some(unconfigured_error(&format!(
                    "set {SERVER_BASE_URL_ENV} before connecting the desktop agent"
                ))),
            ),
            Some(raw) => match normalize_server_base_url(raw) {
                Err(error) => (None, None, AgentConnectionState::Unconfigured, Some(error)),
                Ok(url) => match credentials.load() {
                    Ok(Some(credential)) if credential.server_base_url == url => (
                        Some(url),
                        Some(credential),
                        AgentConnectionState::Offline,
                        None,
                    ),
                    Ok(Some(_)) => (
                        Some(url),
                        None,
                        AgentConnectionState::Unconfigured,
                        Some(unconfigured_error(
                            "saved device pairing belongs to a different server; pair again",
                        )),
                    ),
                    Ok(None) => (
                        Some(url),
                        None,
                        AgentConnectionState::Unconfigured,
                        Some(unconfigured_error("desktop device pairing is required")),
                    ),
                    Err(error) => (
                        Some(url),
                        None,
                        AgentConnectionState::Unconfigured,
                        Some(error),
                    ),
                },
            },
        };

        Self {
            http,
            credentials,
            state: Mutex::new(AgentState {
                server_base_url,
                credential,
                state,
                last_error,
            }),
            room_sync_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn connection_status(&self) -> AgentConnectionStatus {
        let state = self.state.lock().expect("agent state mutex poisoned");
        AgentConnectionStatus {
            state: state.state.clone(),
            server_base_url: state.server_base_url.clone(),
            device_id: state
                .credential
                .as_ref()
                .map(|credential| credential.device_id.clone()),
            last_error_code: state.last_error.as_ref().map(|error| error.code.clone()),
            last_error_message: state.last_error.as_ref().map(|error| error.message.clone()),
        }
    }

    /// Returns the server origin and device token needed to open the realtime Socket.IO client,
    /// or `None` when the desktop is not yet paired. The returned struct is deliberately not
    /// `Serialize`, so the token can never be returned through a Tauri command or logged.
    pub fn realtime_credentials(&self) -> Option<RealtimeCredentials> {
        let state = self.state.lock().expect("agent state mutex poisoned");
        let base_url = state.server_base_url.clone()?;
        let credential = state.credential.as_ref()?;
        Some(RealtimeCredentials {
            base_url,
            device_token: credential.device_token.clone(),
        })
    }

    pub async fn start_pairing(&self, device_name: String) -> Result<PairingSession, AgentError> {
        let device_name = device_name.trim();
        if device_name.is_empty() || device_name.chars().count() > 120 {
            return Err(validation_error(
                "device name must contain between 1 and 120 characters",
            ));
        }
        let base_url = self.require_server_url()?;
        self.set_connecting();

        let result = self
            .send_json::<PairingSessionResponse>(
                self.http
                    .post(format!("{base_url}/v1/pairing-sessions"))
                    .json(&json!({ "deviceName": device_name, "platform": "WINDOWS" })),
            )
            .await
            .and_then(validate_pairing_session);
        if let Err(error) = &result {
            self.record_error(error.clone(), AgentConnectionState::Unconfigured);
        }
        result
    }

    pub async fn poll_pairing(
        &self,
        session_id: String,
        desktop_nonce: String,
    ) -> Result<PairingStatus, AgentError> {
        validate_opaque_value("pairing session id", &session_id, 200)?;
        validate_opaque_value("desktop nonce", &desktop_nonce, 512)?;
        let base_url = self.require_server_url()?;
        let url = format!("{base_url}/v1/pairing-sessions/{session_id}/status");
        let response = self
            .send_json::<PairingStatusResponse>(
                self.http.get(url).query(&[("nonce", desktop_nonce)]),
            )
            .await;

        let response = match response {
            Ok(response) => response,
            Err(error) => {
                self.record_error(error.clone(), AgentConnectionState::Unconfigured);
                return Err(error);
            }
        };

        match response.status.as_str() {
            "PENDING" => {
                self.set_connecting();
                Ok(PairingStatus {
                    status: response.status,
                    device_id: None,
                    expires_at: response.expires_at,
                })
            }
            "CLAIMED" => {
                let device_id = response.device_id.ok_or_else(|| {
                    invalid_response_error("claimed pairing response omitted deviceId")
                })?;
                let device_token = response.device_token.ok_or_else(|| {
                    invalid_response_error("claimed pairing response omitted deviceToken")
                })?;
                let credential = DeviceCredential {
                    server_base_url: base_url,
                    device_id: device_id.clone(),
                    device_token,
                };
                if let Err(error) = self.credentials.save(&credential) {
                    self.record_error(error.clone(), AgentConnectionState::Unconfigured);
                    return Err(error);
                }
                let mut state = self.state.lock().expect("agent state mutex poisoned");
                state.credential = Some(credential);
                state.state = AgentConnectionState::Offline;
                state.last_error = None;
                Ok(PairingStatus {
                    status: response.status,
                    device_id: Some(device_id),
                    expires_at: None,
                })
            }
            _ => Err(invalid_response_error(
                "pairing status must be PENDING or CLAIMED",
            )),
        }
    }

    pub async fn heartbeat(&self, presence: String) -> Result<HeartbeatResult, AgentError> {
        if !matches!(
            presence.as_str(),
            "ONLINE_IDLE" | "ONLINE_SCANNING" | "ONLINE_EXECUTING" | "DEGRADED"
        ) {
            return Err(validation_error(
                "presence must be ONLINE_IDLE, ONLINE_SCANNING, ONLINE_EXECUTING, or DEGRADED",
            ));
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<HeartbeatResponse>(
                self.http
                    .post(format!(
                        "{base_url}/v1/devices/{}/heartbeat",
                        credential.device_id
                    ))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({ "presence": presence })),
            )
            .await
            .and_then(|response| validate_heartbeat_response(&credential.device_id, response));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    /// Revokes only this authenticated desktop device. Any non-success response deliberately keeps
    /// the credential so the caller can retry with the same idempotency key; 401/403 alone cannot
    /// prove that the server committed the revoke transaction.
    pub async fn revoke_self(&self, idempotency_key: String) -> Result<(), AgentError> {
        validate_opaque_value("device revoke idempotency key", &idempotency_key, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_empty(
                self.http
                    .delete(format!("{base_url}/v1/agent/devices/self"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key),
            )
            .await;
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                // A 401/403 only proves that this request was not authorized. It does not prove
                // that the server committed the revoke transaction, so keep the credential and
                // the caller's idempotency key available for an explicit retry.
                self.record_non_authoritative_error(error.clone(), AgentConnectionState::Offline);
                Err(error)
            }
        }
    }

    pub async fn poll_commands(&self) -> Result<Vec<AgentCommand>, AgentError> {
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<ServerCommand>>(
                self.http
                    .get(format!(
                        "{base_url}/v1/devices/{}/commands/pending",
                        credential.device_id
                    ))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|commands| commands.into_iter().map(validate_server_command).collect());
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn ensure_room_for_root(
        &self,
        root_id: String,
        display_name: String,
    ) -> Result<AgentRoomSync, AgentError> {
        validate_root_alias(&root_id)?;
        let display_name = display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 120 {
            return Err(validation_error(
                "managed root display name must contain between 1 and 120 characters",
            ));
        }

        // React StrictMode and manual retries can overlap. Serializing the GET-then-POST
        // sequence prevents duplicate rooms inside one desktop process.
        let _room_sync_guard = self.room_sync_lock.lock().await;
        let (base_url, credential) = self.require_authenticated_config()?;
        let existing = self
            .send_json::<Vec<ServerRoom>>(
                self.http
                    .get(format!("{base_url}/v1/rooms"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|rooms| validate_server_rooms(&credential.device_id, rooms));
        let existing = match existing {
            Ok(rooms) => rooms,
            Err(error) => {
                self.record_error(error.clone(), AgentConnectionState::Offline);
                return Err(error);
            }
        };
        if let Some(room) = existing.into_iter().find(|room| room.root_alias == root_id) {
            self.mark_online();
            return Ok(AgentRoomSync {
                room_id: room.id,
                root_id,
                name: room.name,
                created: false,
            });
        }

        let created = self
            .send_json::<ServerRoom>(
                self.http
                    .post(format!("{base_url}/v1/rooms"))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({
                        "desktopDeviceId": credential.device_id,
                        "name": display_name,
                        "rootAlias": root_id,
                    })),
            )
            .await
            .and_then(|room| validate_server_room(&credential.device_id, room));
        match created {
            Ok(room) if room.root_alias == root_id => {
                self.mark_online();
                Ok(AgentRoomSync {
                    room_id: room.id,
                    root_id,
                    name: room.name,
                    created: true,
                })
            }
            Ok(_) => {
                let error = invalid_response_error(
                    "created room response did not match the managed root alias",
                );
                self.record_error(error.clone(), AgentConnectionState::Offline);
                Err(error)
            }
            Err(error) => {
                self.record_error(error.clone(), AgentConnectionState::Offline);
                Err(error)
            }
        }
    }

    pub async fn root_id_for_room(&self, room_id: String) -> Result<AgentRoomSync, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let rooms = self
            .send_json::<Vec<ServerRoom>>(
                self.http
                    .get(format!("{base_url}/v1/rooms"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|rooms| validate_server_rooms(&credential.device_id, rooms));
        let rooms = match rooms {
            Ok(rooms) => rooms,
            Err(error) => {
                self.record_error(error.clone(), AgentConnectionState::Offline);
                return Err(error);
            }
        };

        let Some(room) = rooms.into_iter().find(|room| room.id == room_id) else {
            let error = invalid_response_error("command room is not registered for this device");
            self.record_error(error.clone(), AgentConnectionState::Offline);
            return Err(error);
        };
        self.mark_online();
        Ok(AgentRoomSync {
            room_id: room.id,
            root_id: room.root_alias,
            name: room.name,
            created: false,
        })
    }

    pub async fn list_rooms(&self) -> Result<Vec<AgentRoomSync>, AgentError> {
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<ServerRoom>>(
                self.http
                    .get(format!("{base_url}/v1/rooms"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|rooms| validate_server_rooms(&credential.device_id, rooms))
            .map(|rooms| {
                rooms
                    .into_iter()
                    .map(|room| AgentRoomSync {
                        room_id: room.id,
                        root_id: room.root_alias,
                        name: room.name,
                        created: false,
                    })
                    .collect()
            });
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn list_chat_sessions(
        &self,
        room_id: String,
    ) -> Result<Vec<AgentChatSession>, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<ChatSessionResponse>>(
                self.http
                    .get(format!("{base_url}/v1/rooms/{room_id}/chat-sessions"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|sessions| validate_chat_sessions(&room_id, sessions));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn chat_quick_view(
        &self,
        room_id: String,
    ) -> Result<AgentChatQuickView, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ChatQuickViewResponse>(
                self.http
                    .get(format!("{base_url}/v1/rooms/{room_id}/chat/quick-view"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|view| validate_chat_quick_view(&room_id, view));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn create_quick_cleanup_suggestion(
        &self,
        room_id: String,
    ) -> Result<AgentChatQuickCleanupResult, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ChatQuickCleanupResponse>(
                self.http
                    .post(format!("{base_url}/v1/rooms/{room_id}/chat/quick-cleanup"))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({})),
            )
            .await
            .and_then(|response| validate_chat_quick_cleanup(&room_id, response));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn create_chat_session(
        &self,
        room_id: String,
        title: Option<String>,
    ) -> Result<AgentChatSession, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let body = match title {
            Some(title) => {
                let trimmed = title.trim();
                if trimmed.is_empty() || trimmed.chars().count() > 120 {
                    return Err(validation_error(
                        "chat session title must contain between 1 and 120 characters",
                    ));
                }
                json!({ "title": trimmed })
            }
            None => json!({}),
        };
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ChatSessionResponse>(
                self.http
                    .post(format!("{base_url}/v1/rooms/{room_id}/chat-sessions"))
                    .bearer_auth(&credential.device_token)
                    .json(&body),
            )
            .await
            .and_then(|session| validate_chat_session(&room_id, session));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn list_chat_messages(
        &self,
        session_id: String,
    ) -> Result<Vec<AgentChatMessage>, AgentError> {
        validate_opaque_value("chat session id", &session_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<ChatMessageResponse>>(
                self.http
                    .get(format!("{base_url}/v1/chat-sessions/{session_id}/messages"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|messages| validate_chat_messages(Some(&session_id), messages));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn mark_chat_session_read(
        &self,
        session_id: String,
        last_read_message_id: Option<String>,
    ) -> Result<AgentChatSession, AgentError> {
        validate_opaque_value("chat session id", &session_id, 200)?;
        if let Some(message_id) = last_read_message_id.as_deref() {
            validate_opaque_value("last read message id", message_id, 200)?;
        }
        let body = match last_read_message_id {
            Some(message_id) => json!({ "lastReadMessageId": message_id }),
            None => json!({}),
        };
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ChatSessionResponse>(
                self.http
                    .post(format!("{base_url}/v1/chat-sessions/{session_id}/read"))
                    .bearer_auth(&credential.device_token)
                    .json(&body),
            )
            .await
            .and_then(|session| validate_chat_session_any_room(session));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn send_chat_message(
        &self,
        session_id: String,
        content: String,
    ) -> Result<AgentChatSendResult, AgentError> {
        validate_opaque_value("chat session id", &session_id, 200)?;
        validate_chat_content(&content)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ChatSendResponse>(
                self.http
                    .post(format!("{base_url}/v1/chat-sessions/{session_id}/messages"))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({ "content": content.trim() })),
            )
            .await
            .and_then(|response| validate_chat_send_result(&session_id, response));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn disconnect_room(
        &self,
        room_id: String,
        idempotency_key: String,
    ) -> Result<(), AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        validate_opaque_value("room disconnect idempotency key", &idempotency_key, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_empty(
                self.http
                    .delete(format!("{base_url}/v1/agent/rooms/{room_id}"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key),
            )
            .await;
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn replay_events(
        &self,
        after: u64,
        limit: u16,
    ) -> Result<Vec<SyncEvent>, AgentError> {
        if !(1..=200).contains(&limit) {
            return Err(validation_error(
                "sync replay limit must be between 1 and 200",
            ));
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<SyncEventResponse>>(
                self.http
                    .get(format!("{base_url}/v1/sync/events"))
                    .bearer_auth(&credential.device_token)
                    .query(&[("after", after.to_string()), ("limit", limit.to_string())]),
            )
            .await
            .and_then(|events| validate_sync_events(after, events));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn update_command_status(
        &self,
        command_id: String,
        status: String,
    ) -> Result<AgentCommand, AgentError> {
        validate_opaque_value("command id", &command_id, 200)?;
        if !matches!(status.as_str(), "DELIVERED" | "ANALYZING" | "FAILED") {
            return Err(validation_error(
                "command status must be DELIVERED, ANALYZING, or FAILED",
            ));
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ServerCommand>(
                self.http
                    .patch(format!(
                        "{base_url}/v1/devices/{}/commands/{command_id}/status",
                        credential.device_id
                    ))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({ "status": status })),
            )
            .await
            .and_then(validate_server_command)
            .and_then(|command| {
                if command.command_id != command_id {
                    return Err(invalid_response_error(
                        "updated command response did not match the requested command id",
                    ));
                }
                Ok(command)
            });
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn submit_proposal(
        &self,
        idempotency_key: String,
        proposal: AgentProposalSubmission,
    ) -> Result<(), AgentError> {
        validate_opaque_value("proposal idempotency key", &idempotency_key, 200)?;
        if proposal.items.is_empty() || proposal.items.len() > 200 {
            return Err(validation_error(
                "proposal submission must contain between 1 and 200 items",
            ));
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_empty(
                self.http
                    .post(format!("{base_url}/v1/agent/proposals"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&proposal),
            )
            .await;
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    /// Creates a room command on the server (`POST /v1/rooms/:id/commands`). The desktop uses this
    /// to start an *autonomous* cleanup: the background loop synthesizes an `ANALYZE` command it
    /// immediately attaches a proposal to, so cleanups the desktop found on its own still reach
    /// mobile through the normal command -> proposal -> decision pipeline (the server requires every
    /// proposal to originate from an `ANALYZING` command). On an idempotent replay the server
    /// returns the existing command with its current status, which the caller uses to detect that a
    /// proposal was already submitted.
    pub async fn create_command(
        &self,
        room_id: String,
        intent: String,
        payload: Value,
        idempotency_key: String,
    ) -> Result<AgentCommand, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        validate_opaque_value("command idempotency key", &idempotency_key, 128)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ServerCommand>(
                self.http
                    .post(format!("{base_url}/v1/rooms/{room_id}/commands"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({ "intent": intent, "payload": payload })),
            )
            .await
            .and_then(validate_server_command);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    /// Counts the proposals still awaiting a decision in a room
    /// (`GET /v1/rooms/:id/proposals/open`). The autonomous cleanup path uses this to avoid piling
    /// up duplicate proposals every tick: it only submits a new one when the room has none open.
    pub async fn open_proposal_count_for_room(&self, room_id: String) -> Result<usize, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<Value>>(
                self.http
                    .get(format!("{base_url}/v1/rooms/{room_id}/proposals/open"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .map(|proposals| proposals.len());
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    /// Fetches approved decisions this device has not yet claimed an execution for.
    ///
    /// The server only ever returns `APPROVE` decisions here (rejected decisions never need a
    /// local execution), and MVP approval always covers every proposal item, so every returned
    /// decision means "execute this entire local proposal snapshot."
    pub async fn pending_decisions(&self) -> Result<Vec<AgentPendingDecision>, AgentError> {
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<PendingDecisionResponse>>(
                self.http
                    .get(format!(
                        "{base_url}/v1/devices/{}/decisions/pending",
                        credential.device_id
                    ))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(validate_pending_decisions);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn pending_file_browse_requests(
        &self,
    ) -> Result<Vec<AgentFileBrowseRequest>, AgentError> {
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<FileBrowseRequestResponse>>(
                self.http
                    .get(format!(
                        "{base_url}/v1/devices/{}/file-browse-requests/pending",
                        credential.device_id
                    ))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(validate_file_browse_requests);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn complete_file_browse_request(
        &self,
        request_id: String,
        result: AgentFileBrowseResult,
    ) -> Result<(), AgentError> {
        validate_opaque_value("file browse request id", &request_id, 200)?;
        if result.entries.len() > 200 {
            return Err(validation_error(
                "file browse result cannot contain more than 200 entries",
            ));
        }
        if result.desktop_generation.is_empty() || result.desktop_generation.len() > 128 {
            return Err(validation_error(
                "file browse desktopGeneration must contain between 1 and 128 characters",
            ));
        }
        if result
            .next_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() > 512)
        {
            return Err(validation_error(
                "file browse nextCursor cannot exceed 512 characters",
            ));
        }

        let (base_url, credential) = self.require_authenticated_config()?;
        let response = self
            .send_empty(
                self.http
                    .post(format!(
                        "{base_url}/v1/agent/file-browse-requests/{request_id}/result"
                    ))
                    .bearer_auth(&credential.device_token)
                    .json(&result),
            )
            .await;
        match &response {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        response
    }

    pub async fn fail_file_browse_request(
        &self,
        request_id: String,
        failure_code: AgentFileBrowseFailureCode,
    ) -> Result<(), AgentError> {
        validate_opaque_value("file browse request id", &request_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let response = self
            .send_empty(
                self.http
                    .post(format!(
                        "{base_url}/v1/agent/file-browse-requests/{request_id}/failure"
                    ))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({ "failureCode": failure_code })),
            )
            .await;
        match &response {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        response
    }

    pub async fn pending_file_transfers(&self) -> Result<Vec<AgentFileTransfer>, AgentError> {
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<Vec<FileTransferResponse>>(
                self.http
                    .get(format!(
                        "{base_url}/v1/devices/{}/file-transfers/pending",
                        credential.device_id
                    ))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(validate_file_transfers);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn request_file_transfer_upload_target(
        &self,
        transfer_id: String,
        source_version: AgentFileTransferSourceVersion,
    ) -> Result<AgentFileTransferUploadTarget, AgentError> {
        validate_opaque_value("file transfer id", &transfer_id, 200)?;
        validate_source_version(&source_version)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<FileTransferUploadTargetResponse>(
                self.http
                    .post(format!(
                        "{base_url}/v1/agent/file-transfers/{transfer_id}/upload-target"
                    ))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({ "sourceVersion": source_version })),
            )
            .await
            .and_then(|response| validate_upload_target_response(&transfer_id, response));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn complete_file_transfer_upload(
        &self,
        transfer_id: String,
        idempotency_key: String,
        size_bytes: u64,
        sha256: String,
    ) -> Result<AgentFileTransfer, AgentError> {
        validate_opaque_value("file transfer id", &transfer_id, 200)?;
        validate_opaque_value(
            "file transfer completion idempotency key",
            &idempotency_key,
            128,
        )?;
        if size_bytes == 0 {
            return Err(validation_error(
                "completed file transfer sizeBytes must be positive",
            ));
        }
        validate_sha256(&sha256)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<FileTransferResponse>(
                self.http
                    .post(format!(
                        "{base_url}/v1/agent/file-transfers/{transfer_id}/complete-upload"
                    ))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({ "sizeBytes": size_bytes, "sha256": sha256 })),
            )
            .await
            .and_then(validate_file_transfer_response)
            .and_then(|transfer| validate_transfer_id_match(&transfer_id, transfer));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn fail_file_transfer(
        &self,
        transfer_id: String,
        failure_code: AgentFileTransferFailureCode,
    ) -> Result<AgentFileTransfer, AgentError> {
        validate_opaque_value("file transfer id", &transfer_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<FileTransferResponse>(
                self.http
                    .post(format!(
                        "{base_url}/v1/agent/file-transfers/{transfer_id}/failure"
                    ))
                    .bearer_auth(&credential.device_token)
                    .json(&json!({ "failureCode": failure_code })),
            )
            .await
            .and_then(validate_file_transfer_response)
            .and_then(|transfer| validate_transfer_id_match(&transfer_id, transfer));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn smart_cache_policy(
        &self,
        room_id: String,
    ) -> Result<AgentSmartCachePolicy, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<SmartCachePolicyResponse>(
                self.http
                    .get(format!("{base_url}/v1/rooms/{room_id}/smart-cache-policy"))
                    .bearer_auth(&credential.device_token),
            )
            .await
            .and_then(|response| validate_smart_cache_policy(&room_id, response));
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn submit_smart_cache_candidates(
        &self,
        idempotency_key: String,
        room_id: String,
        candidates: Vec<AgentSmartCacheCandidate>,
    ) -> Result<AgentSmartCacheCandidateBatchResult, AgentError> {
        validate_opaque_value(
            "smart cache candidate idempotency key",
            &idempotency_key,
            128,
        )?;
        validate_opaque_value("room id", &room_id, 200)?;
        if candidates.is_empty() || candidates.len() > 200 {
            return Err(validation_error(
                "smart cache candidate batch must contain between 1 and 200 candidates",
            ));
        }
        for candidate in &candidates {
            validate_relative_path("smart cache source path", &candidate.source_relative_path)?;
            validate_sha256(&candidate.source_version_hash)?;
            if candidate.size_bytes == 0 {
                return Err(validation_error(
                    "smart cache candidate sizeBytes must be positive",
                ));
            }
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<SmartCacheCandidateBatchResponse>(
                self.http
                    .post(format!("{base_url}/v1/agent/cache-candidates"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({ "roomId": room_id, "candidates": candidates })),
            )
            .await
            .and_then(validate_smart_cache_candidate_batch);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn submit_room_snapshot(
        &self,
        room_id: String,
        snapshot: CleanlinessSnapshot,
    ) -> Result<AgentRoomSnapshot, AgentError> {
        validate_opaque_value("room id", &room_id, 200)?;
        validate_cleanliness_snapshot(&snapshot)?;
        let formula_version = snapshot.formula_version.clone();
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<RoomSnapshotResponse>(
                self.http
                    .post(format!("{base_url}/v1/rooms/{room_id}/snapshots"))
                    .bearer_auth(&credential.device_token)
                    .json(&snapshot),
            )
            .await
            .and_then(|response| {
                validate_room_snapshot_response(&room_id, &formula_version, response)
            });
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn complete_smart_cache_upload(
        &self,
        reservation_id: String,
        idempotency_key: String,
        size_bytes: u64,
        sha256: String,
        usage_score: i64,
        manual_pin: bool,
        encryption_metadata: SmartCacheEncryptionMetadata,
    ) -> Result<AgentCachedFile, AgentError> {
        validate_opaque_value("smart cache reservation id", &reservation_id, 200)?;
        validate_opaque_value(
            "smart cache completion idempotency key",
            &idempotency_key,
            128,
        )?;
        if size_bytes == 0 {
            return Err(validation_error(
                "completed smart cache upload sizeBytes must be positive",
            ));
        }
        validate_sha256(&sha256)?;
        validate_smart_cache_encryption_metadata(&encryption_metadata)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<CachedFileResponse>(
                self.http
                    .post(format!(
                        "{base_url}/v1/agent/cache-uploads/{reservation_id}/complete"
                    ))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({
                        "sizeBytes": size_bytes,
                        "sha256": sha256,
                        "usageScore": usage_score,
                        "manualPin": manual_pin,
                        "encryptionMetadata": encryption_metadata,
                    })),
            )
            .await
            .and_then(validate_cached_file_response);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn mark_smart_cache_stale(
        &self,
        idempotency_key: String,
        room_id: String,
        source_relative_path: Option<String>,
        reason: String,
    ) -> Result<AgentSmartCacheStaleResult, AgentError> {
        validate_opaque_value("smart cache stale idempotency key", &idempotency_key, 128)?;
        validate_opaque_value("room id", &room_id, 200)?;
        validate_smart_cache_stale_reason(&reason)?;
        match (&source_relative_path, reason.as_str()) {
            (Some(path), "SOURCE_CHANGED" | "SOURCE_REMOVED") => {
                validate_relative_path("smart cache stale source path", path)?;
            }
            (None, "REINDEXED") => {}
            (None, _) => {
                return Err(validation_error(
                    "smart cache source changes must include sourceRelativePath",
                ));
            }
            (Some(_), "REINDEXED") => {
                return Err(validation_error(
                    "smart cache REINDEXED stale reports must not include sourceRelativePath",
                ));
            }
            _ => unreachable!("reason was validated above"),
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<SmartCacheStaleResponse>(
                self.http
                    .post(format!("{base_url}/v1/agent/cached-files/stale"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({
                        "roomId": room_id.clone(),
                        "sourceRelativePath": source_relative_path.clone(),
                        "reason": reason.clone(),
                    })),
            )
            .await
            .and_then(|response| {
                validate_smart_cache_stale_response(
                    &room_id,
                    source_relative_path.as_deref(),
                    &reason,
                    response,
                )
            });
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub async fn cancel_smart_cache_reservation(
        &self,
        reservation_id: String,
    ) -> Result<(), AgentError> {
        validate_opaque_value("smart cache reservation id", &reservation_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_empty(
                self.http
                    .delete(format!(
                        "{base_url}/v1/agent/cache-uploads/{reservation_id}"
                    ))
                    .bearer_auth(&credential.device_token),
            )
            .await;
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    /// Claims an approved decision for local execution. Uses the decision id as the
    /// idempotency key so a retry after a crash or network failure cannot create a second
    /// execution for the same approval.
    pub async fn create_execution(
        &self,
        proposal_id: String,
        decision_id: String,
    ) -> Result<AgentExecution, AgentError> {
        validate_opaque_value("proposal id", &proposal_id, 200)?;
        validate_opaque_value("decision id", &decision_id, 200)?;
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ExecutionResponse>(
                self.http
                    .post(format!("{base_url}/v1/agent/executions"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", decision_id.clone())
                    .json(&json!({
                        "proposalId": proposal_id,
                        "decisionId": decision_id,
                        "desktopDeviceId": credential.device_id,
                    })),
            )
            .await
            .and_then(validate_execution_response);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    /// Uploads the terminal outcome of a claimed execution. Uses the execution id as the
    /// idempotency key so a retry of the same outcome after a network failure replays instead
    /// of being rejected as a conflicting update.
    pub async fn update_execution(
        &self,
        execution_id: String,
        status: String,
        result_summary: Value,
    ) -> Result<AgentExecution, AgentError> {
        validate_opaque_value("execution id", &execution_id, 200)?;
        if !matches!(
            status.as_str(),
            "SUCCEEDED" | "PARTIALLY_SUCCEEDED" | "FAILED" | "STALE" | "ROLLED_BACK"
        ) {
            return Err(validation_error(
                "execution status must be SUCCEEDED, PARTIALLY_SUCCEEDED, FAILED, STALE, or ROLLED_BACK",
            ));
        }
        if !result_summary.is_object() {
            return Err(validation_error(
                "execution result summary must be a JSON object",
            ));
        }
        let (base_url, credential) = self.require_authenticated_config()?;
        let result = self
            .send_json::<ExecutionResponse>(
                self.http
                    .patch(format!("{base_url}/v1/agent/executions/{execution_id}"))
                    .bearer_auth(&credential.device_token)
                    .header("Idempotency-Key", execution_id.clone())
                    .json(&json!({
                        "status": status,
                        "resultSummary": result_summary,
                    })),
            )
            .await
            .and_then(validate_execution_response);
        match &result {
            Ok(_) => self.mark_online(),
            Err(error) => self.record_error(error.clone(), AgentConnectionState::Offline),
        }
        result
    }

    pub fn forget_device(&self) -> Result<AgentConnectionStatus, AgentError> {
        self.credentials.delete()?;
        let mut state = self.state.lock().expect("agent state mutex poisoned");
        state.credential = None;
        state.state = AgentConnectionState::Unconfigured;
        state.last_error = Some(unconfigured_error("desktop device pairing is required"));
        drop(state);
        Ok(self.connection_status())
    }

    fn require_server_url(&self) -> Result<String, AgentError> {
        self.state
            .lock()
            .expect("agent state mutex poisoned")
            .server_base_url
            .clone()
            .ok_or_else(|| {
                unconfigured_error(&format!(
                    "set {SERVER_BASE_URL_ENV} before connecting the desktop agent"
                ))
            })
    }

    fn require_authenticated_config(&self) -> Result<(String, DeviceCredential), AgentError> {
        let state = self.state.lock().expect("agent state mutex poisoned");
        let base_url = state.server_base_url.clone().ok_or_else(|| {
            unconfigured_error(&format!(
                "set {SERVER_BASE_URL_ENV} before connecting the desktop agent"
            ))
        })?;
        let credential = state
            .credential
            .clone()
            .ok_or_else(|| unconfigured_error("desktop device pairing is required"))?;
        Ok((base_url, credential))
    }

    fn set_connecting(&self) {
        let mut state = self.state.lock().expect("agent state mutex poisoned");
        state.state = AgentConnectionState::Connecting;
        state.last_error = None;
    }

    fn mark_online(&self) {
        let mut state = self.state.lock().expect("agent state mutex poisoned");
        state.state = AgentConnectionState::Online;
        state.last_error = None;
    }

    fn record_error(&self, error: AgentError, connection_state: AgentConnectionState) {
        if error.is_confirmed_device_absent() {
            let _ = self.credentials.delete();
            let mut state = self.state.lock().expect("agent state mutex poisoned");
            state.credential = None;
            state.state = AgentConnectionState::Revoked;
            state.last_error = Some(error);
            return;
        }

        let mut state = self.state.lock().expect("agent state mutex poisoned");
        state.state = connection_state;
        state.last_error = Some(error);
    }

    fn record_non_authoritative_error(
        &self,
        error: AgentError,
        connection_state: AgentConnectionState,
    ) {
        let mut state = self.state.lock().expect("agent state mutex poisoned");
        state.state = connection_state;
        state.last_error = Some(error);
    }

    async fn send_json<T>(&self, request: reqwest::RequestBuilder) -> Result<T, AgentError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = request.send().await.map_err(|_| AgentError {
            code: AgentErrorCode::TransportUnavailable,
            message: "cannot reach the MouseKeeper server".to_string(),
            server_code: None,
        })?;
        let status = response.status();
        if !status.is_success() {
            let server_error = response.json::<ServerErrorResponse>().await.ok();
            return Err(http_error(status, server_error));
        }
        response
            .json::<T>()
            .await
            .map_err(|_| invalid_response_error("server returned an unexpected response shape"))
    }

    async fn send_empty(&self, request: reqwest::RequestBuilder) -> Result<(), AgentError> {
        let response = request.send().await.map_err(|_| AgentError {
            code: AgentErrorCode::TransportUnavailable,
            message: "cannot reach the MouseKeeper server".to_string(),
            server_code: None,
        })?;
        let status = response.status();
        if !status.is_success() {
            let server_error = response.json::<ServerErrorResponse>().await.ok();
            return Err(http_error(status, server_error));
        }
        Ok(())
    }

    #[cfg(test)]
    fn for_test(server_base_url: Option<&str>, credentials: Arc<dyn CredentialStore>) -> Self {
        Self::new(server_base_url, credentials)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingSessionResponse {
    session_id: String,
    desktop_nonce: String,
    code: String,
    expires_at: String,
}

impl From<PairingSessionResponse> for PairingSession {
    fn from(value: PairingSessionResponse) -> Self {
        Self {
            session_id: value.session_id,
            desktop_nonce: value.desktop_nonce,
            code: value.code,
            expires_at: value.expires_at,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingStatusResponse {
    status: String,
    expires_at: Option<String>,
    device_id: Option<String>,
    device_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeartbeatResponse {
    device_id: String,
    presence: String,
    ttl_seconds: u64,
}

impl From<HeartbeatResponse> for HeartbeatResult {
    fn from(value: HeartbeatResponse) -> Self {
        Self {
            device_id: value.device_id,
            presence: value.presence,
            ttl_seconds: value.ttl_seconds,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerCommand {
    id: String,
    intent: String,
    room_id: String,
    status: String,
    payload: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerRoom {
    id: String,
    desktop_device_id: String,
    name: String,
    root_alias: String,
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSessionResponse {
    id: String,
    room_id: String,
    title: String,
    status: String,
    created_at: String,
    updated_at: String,
    message_preview: String,
    unread_count: Option<u64>,
    pending_action_count: Option<u64>,
    last_read_message_id: Option<String>,
    read_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageResponse {
    id: String,
    room_id: String,
    session_id: Option<String>,
    sender_type: String,
    message_type: String,
    content: String,
    structured_payload: Value,
    command_id: Option<String>,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSendResponse {
    message: ChatMessageResponse,
    assistant: Option<ChatMessageResponse>,
    ai_status: String,
    ai: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatQuickPromptResponse {
    id: String,
    label: String,
    prompt: String,
    category: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatQuickHistoryResponse {
    message_id: String,
    session_id: String,
    session_title: String,
    sender_type: String,
    message_type: String,
    content: String,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatQuickSuggestionResponse {
    message_id: String,
    session_id: String,
    session_title: String,
    message_type: String,
    content: String,
    draft_id: String,
    status: String,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatQuickViewResponse {
    prompts: Vec<ChatQuickPromptResponse>,
    sessions: Vec<ChatSessionResponse>,
    history: Vec<ChatQuickHistoryResponse>,
    pending_suggestions: Vec<ChatQuickSuggestionResponse>,
    unread_count: u64,
    pending_action_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatQuickCleanupResponse {
    session: ChatSessionResponse,
    message: ChatMessageResponse,
    assistant: Option<ChatMessageResponse>,
    ai_status: String,
    ai: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncEventResponse {
    event_id: String,
    event_type: String,
    schema_version: u64,
    correlation_id: String,
    aggregate_type: String,
    aggregate_id: String,
    device_id: Option<String>,
    room_id: Option<String>,
    sequence: u64,
    occurred_at: String,
    payload: Value,
}

impl From<SyncEventResponse> for SyncEvent {
    fn from(value: SyncEventResponse) -> Self {
        Self {
            event_id: value.event_id,
            event_type: value.event_type,
            schema_version: value.schema_version,
            correlation_id: value.correlation_id,
            aggregate_type: value.aggregate_type,
            aggregate_id: value.aggregate_id,
            device_id: value.device_id,
            room_id: value.room_id,
            sequence: value.sequence,
            occurred_at: value.occurred_at,
            payload: value.payload,
        }
    }
}

impl From<ServerCommand> for AgentCommand {
    fn from(value: ServerCommand) -> Self {
        Self {
            command_id: value.id,
            command_type: value.intent,
            room_id: value.room_id,
            status: value.status,
            payload: value.payload,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecisionResponse {
    id: String,
    proposal_id: String,
    decision_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProposalResponse {
    id: String,
    room_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProposalItemResponse {
    id: String,
    proposal_id: String,
    item_order: i64,
    action_type: String,
    source_relative_path: Option<String>,
    destination_relative_path: Option<String>,
    reason_code: String,
    precondition: Value,
    conflict_state: String,
}

#[derive(Deserialize)]
struct PendingDecisionResponse {
    decision: DecisionResponse,
    proposal: ProposalResponse,
    items: Vec<ProposalItemResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileBrowseRequestResponse {
    id: String,
    room_id: String,
    relative_directory: String,
    cursor: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default = "default_file_browse_limit")]
    limit: usize,
    #[serde(default)]
    search_scope: AgentFileSearchScope,
    status: String,
}

fn default_file_browse_limit() -> usize {
    200
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileTransferResponse {
    id: String,
    room_id: String,
    source_relative_path: String,
    status: String,
    expires_at: String,
    size_bytes: Option<u64>,
    sha256: Option<String>,
    failure_code: Option<AgentFileTransferFailureCode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileTransferUploadTargetResponse {
    transfer_id: String,
    upload_url: String,
    expires_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmartCachePolicyResponse {
    room_id: String,
    enabled: bool,
    quota_bytes: u64,
    max_file_bytes: u64,
    excluded_patterns: Vec<String>,
    #[serde(default)]
    pinned_patterns: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmartCacheReservationResponse {
    reservation_id: String,
    status: String,
    source_relative_path: String,
    source_version_hash: String,
    size_bytes: u64,
    upload_url: Option<String>,
    expires_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmartCacheCandidateBatchResponse {
    batch_id: String,
    approved: Vec<SmartCacheReservationResponse>,
    rejected_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedFileResponse {
    id: String,
    source_relative_path: String,
    source_version_hash: String,
    size_bytes: u64,
    sha256: String,
    freshness_status: String,
    #[serde(default)]
    encryption_metadata: Option<SmartCacheEncryptionMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmartCacheStaleResponse {
    room_id: String,
    source_relative_path: Option<String>,
    reason: String,
    stale_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomSnapshotResponse {
    id: String,
    room_id: String,
    #[serde(default)]
    formula_version: Option<String>,
    score: u8,
    metrics: Value,
    calculated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionResponse {
    id: String,
    status: String,
}

#[derive(Deserialize)]
struct ServerErrorResponse {
    code: Option<String>,
    message: Option<String>,
}

fn normalize_server_base_url(raw: &str) -> Result<String, AgentError> {
    let mut url = Url::parse(raw.trim())
        .map_err(|_| validation_error("server base URL must be a valid http or https URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (url.path() != "/" && !url.path().is_empty())
    {
        return Err(validation_error(
            "server base URL must be an http(s) origin without credentials, path, query, or fragment",
        ));
    }
    let host = url.host_str().unwrap_or_default();
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false);
    if url.scheme() == "http" && !is_loopback {
        return Err(validation_error(
            "plain HTTP is allowed only for a loopback development server; use HTTPS otherwise",
        ));
    }
    url.set_path("");
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn validate_opaque_value(name: &str, value: &str, max_length: usize) -> Result<(), AgentError> {
    if value.is_empty()
        || value.len() > max_length
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(validation_error(&format!("{name} has an invalid format")));
    }
    Ok(())
}

fn validate_root_alias(value: &str) -> Result<(), AgentError> {
    if value.is_empty()
        || value.len() > 120
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
        })
    {
        return Err(validation_error(
            "managed root id has an invalid room alias format",
        ));
    }
    Ok(())
}

fn validate_server_rooms(
    expected_device_id: &str,
    rooms: Vec<ServerRoom>,
) -> Result<Vec<ServerRoom>, AgentError> {
    // `GET /v1/rooms` is scoped to the user, not the device, so the response can legitimately
    // include rooms owned by the user's other desktop devices (e.g. a previous pairing left
    // stale rooms behind). Those foreign rooms are not an error for this device: silently drop
    // them and only validate the rooms this device actually owns. Hard-failing on a foreign room
    // would break room sync entirely after re-pairing.
    rooms
        .into_iter()
        .filter(|room| room.desktop_device_id == expected_device_id)
        .map(|room| validate_server_room(expected_device_id, room))
        .collect()
}

fn validate_server_room(
    expected_device_id: &str,
    room: ServerRoom,
) -> Result<ServerRoom, AgentError> {
    if room.id.is_empty()
        || room.desktop_device_id != expected_device_id
        || room.name.trim().is_empty()
        || room.name.chars().count() > 120
        || room.root_alias.trim().is_empty()
        || room.root_alias.chars().count() > 120
        || room.status != "ACTIVE"
    {
        return Err(invalid_response_error("room failed response validation"));
    }
    Ok(room)
}

fn validate_chat_sessions(
    expected_room_id: &str,
    responses: Vec<ChatSessionResponse>,
) -> Result<Vec<AgentChatSession>, AgentError> {
    responses
        .into_iter()
        .map(|session| validate_chat_session(expected_room_id, session))
        .collect()
}

fn validate_chat_session(
    expected_room_id: &str,
    response: ChatSessionResponse,
) -> Result<AgentChatSession, AgentError> {
    if response.room_id != expected_room_id {
        return Err(invalid_response_error(
            "chat session failed response validation",
        ));
    }
    validate_chat_session_any_room(response)
}

fn validate_chat_session_any_room(
    response: ChatSessionResponse,
) -> Result<AgentChatSession, AgentError> {
    if response.id.is_empty()
        || response.room_id.is_empty()
        || response.title.trim().is_empty()
        || response.title.chars().count() > 120
        || response.status != "ACTIVE"
        || response.created_at.is_empty()
        || response.updated_at.is_empty()
        || response.message_preview.chars().count() > 120
    {
        return Err(invalid_response_error(
            "chat session failed response validation",
        ));
    }
    Ok(AgentChatSession {
        session_id: response.id,
        room_id: response.room_id,
        title: response.title,
        status: response.status,
        created_at: response.created_at,
        updated_at: response.updated_at,
        message_preview: response.message_preview,
        unread_count: response.unread_count.unwrap_or(0),
        pending_action_count: response.pending_action_count.unwrap_or(0),
        last_read_message_id: response.last_read_message_id,
        read_at: response.read_at,
    })
}

fn validate_chat_messages(
    expected_session_id: Option<&str>,
    responses: Vec<ChatMessageResponse>,
) -> Result<Vec<AgentChatMessage>, AgentError> {
    responses
        .into_iter()
        .map(|message| validate_chat_message(expected_session_id, message))
        .collect()
}

fn validate_chat_message(
    expected_session_id: Option<&str>,
    response: ChatMessageResponse,
) -> Result<AgentChatMessage, AgentError> {
    let session_matches = match expected_session_id {
        Some(expected) => response.session_id.as_deref() == Some(expected),
        None => response.session_id.is_some(),
    };
    if response.id.is_empty()
        || response.room_id.is_empty()
        || !session_matches
        || !matches!(response.sender_type.as_str(), "USER" | "ASSISTANT")
        || !matches!(
            response.message_type.as_str(),
            "TEXT" | "COMMAND_DRAFT" | "RULE_DRAFT" | "QUERY_RESULT" | "EXECUTION_RESULT"
        )
        || response.content.chars().count() > 2_000
        || response.created_at.is_empty()
    {
        return Err(invalid_response_error(
            "chat message failed response validation",
        ));
    }
    Ok(AgentChatMessage {
        message_id: response.id,
        room_id: response.room_id,
        session_id: response.session_id,
        sender_type: response.sender_type,
        message_type: response.message_type,
        content: response.content,
        structured_payload: response.structured_payload,
        command_id: response.command_id,
        created_at: response.created_at,
    })
}

fn validate_chat_send_result(
    expected_session_id: &str,
    response: ChatSendResponse,
) -> Result<AgentChatSendResult, AgentError> {
    let message = validate_chat_message(Some(expected_session_id), response.message)?;
    let assistant = response
        .assistant
        .map(|message| validate_chat_message(Some(expected_session_id), message))
        .transpose()?;
    if response.ai_status.is_empty() || !response.ai.is_object() {
        return Err(invalid_response_error(
            "chat AI result failed response validation",
        ));
    }
    Ok(AgentChatSendResult {
        message,
        assistant,
        ai_status: response.ai_status,
        ai: response.ai,
    })
}

fn validate_chat_quick_view(
    expected_room_id: &str,
    response: ChatQuickViewResponse,
) -> Result<AgentChatQuickView, AgentError> {
    if response.prompts.is_empty()
        || response.prompts.len() > 12
        || response.sessions.len() > 5
        || response.history.len() > 12
        || response.pending_suggestions.len() > 12
    {
        return Err(invalid_response_error(
            "chat quick view failed response validation",
        ));
    }
    let prompts = response
        .prompts
        .into_iter()
        .map(validate_chat_quick_prompt)
        .collect::<Result<Vec<_>, _>>()?;
    let sessions = validate_chat_sessions(expected_room_id, response.sessions)?;
    let history = response
        .history
        .into_iter()
        .map(validate_chat_quick_history)
        .collect::<Result<Vec<_>, _>>()?;
    let pending_suggestions = response
        .pending_suggestions
        .into_iter()
        .map(validate_chat_quick_suggestion)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AgentChatQuickView {
        prompts,
        sessions,
        history,
        pending_suggestions,
        unread_count: response.unread_count,
        pending_action_count: response.pending_action_count,
    })
}

fn validate_chat_quick_cleanup(
    expected_room_id: &str,
    response: ChatQuickCleanupResponse,
) -> Result<AgentChatQuickCleanupResult, AgentError> {
    let session = validate_chat_session(expected_room_id, response.session)?;
    let message = validate_chat_message(Some(&session.session_id), response.message)?;
    let assistant = response
        .assistant
        .map(|message| validate_chat_message(Some(&session.session_id), message))
        .transpose()?;
    if response.ai_status != "READY" || !response.ai.is_object() {
        return Err(invalid_response_error(
            "chat quick cleanup failed response validation",
        ));
    }
    Ok(AgentChatQuickCleanupResult {
        session,
        message,
        assistant,
        ai_status: response.ai_status,
        ai: response.ai,
    })
}

fn validate_chat_quick_prompt(
    response: ChatQuickPromptResponse,
) -> Result<AgentChatQuickPrompt, AgentError> {
    if response.id.trim().is_empty()
        || response.id.chars().count() > 80
        || response.label.trim().is_empty()
        || response.label.chars().count() > 80
        || response.prompt.trim().is_empty()
        || response.prompt.chars().count() > 500
        || !matches!(
            response.category.as_str(),
            "QUERY" | "COMMAND" | "RULE" | "CLEANUP"
        )
    {
        return Err(invalid_response_error(
            "chat quick prompt failed response validation",
        ));
    }
    Ok(AgentChatQuickPrompt {
        id: response.id,
        label: response.label,
        prompt: response.prompt,
        category: response.category,
    })
}

fn validate_chat_quick_history(
    response: ChatQuickHistoryResponse,
) -> Result<AgentChatQuickHistoryItem, AgentError> {
    if response.message_id.trim().is_empty()
        || response.session_id.trim().is_empty()
        || response.session_title.trim().is_empty()
        || response.session_title.chars().count() > 120
        || !matches!(response.sender_type.as_str(), "USER" | "ASSISTANT" | "SYSTEM")
        || !matches!(
            response.message_type.as_str(),
            "TEXT" | "COMMAND_DRAFT" | "RULE_DRAFT" | "QUERY_RESULT" | "EXECUTION_RESULT"
        )
        || response.content.trim().is_empty()
        || response.content.chars().count() > 2_000
        || response.created_at.trim().is_empty()
    {
        return Err(invalid_response_error(
            "chat quick history failed response validation",
        ));
    }
    Ok(AgentChatQuickHistoryItem {
        message_id: response.message_id,
        session_id: response.session_id,
        session_title: response.session_title,
        sender_type: response.sender_type,
        message_type: response.message_type,
        content: response.content,
        created_at: response.created_at,
    })
}

fn validate_chat_quick_suggestion(
    response: ChatQuickSuggestionResponse,
) -> Result<AgentChatQuickSuggestion, AgentError> {
    if response.message_id.trim().is_empty()
        || response.session_id.trim().is_empty()
        || response.session_title.trim().is_empty()
        || response.session_title.chars().count() > 120
        || !matches!(response.message_type.as_str(), "COMMAND_DRAFT" | "RULE_DRAFT")
        || response.content.trim().is_empty()
        || response.content.chars().count() > 2_000
        || response.draft_id.trim().is_empty()
        || response.status.trim().is_empty()
        || response.status.chars().count() > 40
        || response.created_at.trim().is_empty()
    {
        return Err(invalid_response_error(
            "chat quick suggestion failed response validation",
        ));
    }
    Ok(AgentChatQuickSuggestion {
        message_id: response.message_id,
        session_id: response.session_id,
        session_title: response.session_title,
        message_type: response.message_type,
        content: response.content,
        draft_id: response.draft_id,
        status: response.status,
        created_at: response.created_at,
    })
}

fn validate_chat_content(content: &str) -> Result<(), AgentError> {
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 2_000 {
        return Err(validation_error(
            "chat message content must contain between 1 and 2000 characters",
        ));
    }
    Ok(())
}

fn validate_sync_events(
    after: u64,
    events: Vec<SyncEventResponse>,
) -> Result<Vec<SyncEvent>, AgentError> {
    let mut previous = after;
    let mut validated = Vec::with_capacity(events.len());
    for event in events {
        if event.sequence <= previous
            || event.event_id.is_empty()
            || event.event_type.is_empty()
            || event.schema_version == 0
            || event.correlation_id.is_empty()
            || event.aggregate_type.is_empty()
            || event.aggregate_id.is_empty()
            || event.occurred_at.is_empty()
            || !event.payload.is_object()
        {
            return Err(invalid_response_error(
                "sync replay event failed envelope validation",
            ));
        }
        previous = event.sequence;
        validated.push(event.into());
    }
    Ok(validated)
}

fn validate_pairing_session(
    response: PairingSessionResponse,
) -> Result<PairingSession, AgentError> {
    if response.session_id.is_empty()
        || response.desktop_nonce.is_empty()
        || response.expires_at.is_empty()
        || response.code.len() != 6
        || !response
            .code
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(invalid_response_error(
            "pairing session failed response validation",
        ));
    }
    Ok(response.into())
}

fn validate_heartbeat_response(
    expected_device_id: &str,
    response: HeartbeatResponse,
) -> Result<HeartbeatResult, AgentError> {
    if response.device_id != expected_device_id
        || !matches!(
            response.presence.as_str(),
            "ONLINE_IDLE" | "ONLINE_SCANNING" | "ONLINE_EXECUTING" | "DEGRADED"
        )
        || response.ttl_seconds == 0
    {
        return Err(invalid_response_error(
            "heartbeat failed response validation",
        ));
    }
    Ok(response.into())
}

fn validate_server_command(response: ServerCommand) -> Result<AgentCommand, AgentError> {
    if response.id.is_empty()
        || response.intent.is_empty()
        || response.room_id.is_empty()
        || !matches!(
            response.status.as_str(),
            "QUEUED"
                | "DELIVERED"
                | "ANALYZING"
                | "PROPOSAL_READY"
                | "WAITING_APPROVAL"
                | "APPROVED"
                | "REJECTED"
                | "EXPIRED"
                | "EXECUTING"
                | "SUCCEEDED"
                | "PARTIALLY_SUCCEEDED"
                | "FAILED"
                | "STALE"
        )
        || !response.payload.is_object()
    {
        return Err(invalid_response_error("command failed response validation"));
    }
    Ok(response.into())
}

fn validate_pending_decisions(
    responses: Vec<PendingDecisionResponse>,
) -> Result<Vec<AgentPendingDecision>, AgentError> {
    responses
        .into_iter()
        .map(validate_pending_decision)
        .collect()
}

fn validate_pending_decision(
    response: PendingDecisionResponse,
) -> Result<AgentPendingDecision, AgentError> {
    if response.decision.id.is_empty()
        || response.decision.proposal_id != response.proposal.id
        || response.decision.decision_type != "APPROVE"
        || response.proposal.id.is_empty()
        || response.proposal.room_id.is_empty()
        || response.items.is_empty()
    {
        return Err(invalid_response_error(
            "pending decision failed response validation",
        ));
    }
    let items = response
        .items
        .into_iter()
        .map(|item| validate_proposal_item(&response.proposal.id, item))
        .collect::<Result<Vec<_>, AgentError>>()?;
    Ok(AgentPendingDecision {
        decision_id: response.decision.id,
        proposal_id: response.proposal.id,
        room_id: response.proposal.room_id,
        items,
    })
}

fn validate_file_browse_requests(
    responses: Vec<FileBrowseRequestResponse>,
) -> Result<Vec<AgentFileBrowseRequest>, AgentError> {
    responses
        .into_iter()
        .map(validate_file_browse_request)
        .collect()
}

fn validate_file_browse_request(
    response: FileBrowseRequestResponse,
) -> Result<AgentFileBrowseRequest, AgentError> {
    if response.id.is_empty()
        || response.room_id.is_empty()
        || response.relative_directory.len() > 1024
        || response
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() > 512)
        || response.query.as_ref().is_some_and(|query| {
            !(2..=100).contains(&query.trim().chars().count()) || query != query.trim()
        })
        || response.limit == 0
        || response.limit > 200
        || !file_browse_extensions_are_valid(&response.extensions)
        || (response.query.is_none() && !response.extensions.is_empty())
        || response.status != "REQUESTED"
    {
        return Err(invalid_response_error(
            "file browse request failed response validation",
        ));
    }
    Ok(AgentFileBrowseRequest {
        request_id: response.id,
        room_id: response.room_id,
        relative_directory: response.relative_directory,
        cursor: response.cursor,
        query: response.query,
        extensions: response.extensions,
        limit: response.limit,
        search_scope: response.search_scope,
    })
}

fn file_browse_extensions_are_valid(extensions: &[String]) -> bool {
    extensions.is_empty()
        || (extensions.len() <= 50
            && extensions.iter().all(|extension| {
                let bytes = extension.as_bytes();
                bytes.len() >= 2
                    && bytes[0] == b'.'
                    && bytes[1..].iter().all(|byte| byte.is_ascii_alphanumeric())
            }))
}

fn validate_file_transfers(
    responses: Vec<FileTransferResponse>,
) -> Result<Vec<AgentFileTransfer>, AgentError> {
    responses
        .into_iter()
        .map(validate_file_transfer_response)
        .collect()
}

fn validate_file_transfer_response(
    response: FileTransferResponse,
) -> Result<AgentFileTransfer, AgentError> {
    if response.id.is_empty()
        || response.room_id.is_empty()
        || response.source_relative_path.is_empty()
        || response.source_relative_path.len() > 1024
        || response.expires_at.is_empty()
        || !matches!(
            response.status.as_str(),
            "REQUESTED" | "UPLOADING" | "READY" | "FAILED" | "COMPLETED" | "CANCELLED" | "EXPIRED"
        )
        || response
            .sha256
            .as_ref()
            .is_some_and(|sha256| !is_valid_sha256(sha256))
    {
        return Err(invalid_response_error(
            "file transfer failed response validation",
        ));
    }
    Ok(AgentFileTransfer {
        transfer_id: response.id,
        room_id: response.room_id,
        source_relative_path: response.source_relative_path,
        status: response.status,
        expires_at: response.expires_at,
        size_bytes: response.size_bytes,
        sha256: response.sha256,
        failure_code: response.failure_code,
    })
}

fn validate_transfer_id_match(
    expected_transfer_id: &str,
    transfer: AgentFileTransfer,
) -> Result<AgentFileTransfer, AgentError> {
    if transfer.transfer_id != expected_transfer_id {
        return Err(invalid_response_error(
            "file transfer response did not match the requested transfer id",
        ));
    }
    Ok(transfer)
}

fn validate_upload_target_response(
    expected_transfer_id: &str,
    response: FileTransferUploadTargetResponse,
) -> Result<AgentFileTransferUploadTarget, AgentError> {
    if response.transfer_id != expected_transfer_id || response.expires_at.is_empty() {
        return Err(invalid_response_error(
            "file transfer upload target failed response validation",
        ));
    }
    let url = Url::parse(&response.upload_url)
        .map_err(|_| invalid_response_error("file transfer upload target URL is invalid"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(invalid_response_error(
            "file transfer upload target must be an http(s) URL",
        ));
    }
    Ok(AgentFileTransferUploadTarget {
        transfer_id: response.transfer_id,
        upload_url: response.upload_url,
        expires_at: response.expires_at,
    })
}

fn validate_smart_cache_policy(
    expected_room_id: &str,
    response: SmartCachePolicyResponse,
) -> Result<AgentSmartCachePolicy, AgentError> {
    if response.room_id != expected_room_id
        || response.quota_bytes == 0
        || response.max_file_bytes == 0
        || response.max_file_bytes > response.quota_bytes
    {
        return Err(invalid_response_error(
            "smart cache policy failed response validation",
        ));
    }
    Ok(AgentSmartCachePolicy {
        room_id: response.room_id,
        enabled: response.enabled,
        quota_bytes: response.quota_bytes,
        max_file_bytes: response.max_file_bytes,
        excluded_patterns: response.excluded_patterns,
        pinned_patterns: response.pinned_patterns,
    })
}

fn validate_smart_cache_candidate_batch(
    response: SmartCacheCandidateBatchResponse,
) -> Result<AgentSmartCacheCandidateBatchResult, AgentError> {
    if response.batch_id.is_empty() {
        return Err(invalid_response_error(
            "smart cache candidate batch omitted batch id",
        ));
    }
    let approved = response
        .approved
        .into_iter()
        .map(validate_smart_cache_reservation)
        .collect::<Result<Vec<_>, AgentError>>()?;
    Ok(AgentSmartCacheCandidateBatchResult {
        batch_id: response.batch_id,
        approved,
        rejected_count: response.rejected_count,
    })
}

fn validate_smart_cache_reservation(
    response: SmartCacheReservationResponse,
) -> Result<AgentSmartCacheReservation, AgentError> {
    validate_relative_path(
        "smart cache reservation path",
        &response.source_relative_path,
    )?;
    validate_sha256(&response.source_version_hash)?;
    if response.reservation_id.is_empty()
        || response.expires_at.is_empty()
        || response.size_bytes == 0
        || !matches!(response.status.as_str(), "RESERVED" | "COMPLETED")
    {
        return Err(invalid_response_error(
            "smart cache reservation failed response validation",
        ));
    }
    if response.status == "RESERVED" {
        let upload_url = response
            .upload_url
            .as_ref()
            .ok_or_else(|| invalid_response_error("smart cache reservation omitted upload URL"))?;
        let url = Url::parse(upload_url)
            .map_err(|_| invalid_response_error("smart cache upload URL is invalid"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(invalid_response_error(
                "smart cache upload target must be an http(s) URL",
            ));
        }
    }
    Ok(AgentSmartCacheReservation {
        reservation_id: response.reservation_id,
        status: response.status,
        source_relative_path: response.source_relative_path,
        source_version_hash: response.source_version_hash,
        size_bytes: response.size_bytes,
        upload_url: response.upload_url,
        expires_at: response.expires_at,
    })
}

fn validate_cached_file_response(
    response: CachedFileResponse,
) -> Result<AgentCachedFile, AgentError> {
    validate_relative_path("cached file path", &response.source_relative_path)?;
    validate_sha256(&response.source_version_hash)?;
    validate_sha256(&response.sha256)?;
    if let Some(metadata) = &response.encryption_metadata {
        validate_smart_cache_encryption_metadata(metadata)?;
    }
    if response.id.is_empty()
        || response.size_bytes == 0
        || !matches!(
            response.freshness_status.as_str(),
            "VERIFIED_CURRENT" | "UNVERIFIED_OFFLINE" | "STALE"
        )
    {
        return Err(invalid_response_error(
            "cached file failed response validation",
        ));
    }
    Ok(AgentCachedFile {
        cached_file_id: response.id,
        source_relative_path: response.source_relative_path,
        source_version_hash: response.source_version_hash,
        size_bytes: response.size_bytes,
        sha256: response.sha256,
        freshness_status: response.freshness_status,
        encryption_metadata: response.encryption_metadata,
    })
}

fn validate_smart_cache_stale_reason(reason: &str) -> Result<(), AgentError> {
    if !matches!(reason, "SOURCE_CHANGED" | "SOURCE_REMOVED" | "REINDEXED") {
        return Err(validation_error(
            "smart cache stale reason must be SOURCE_CHANGED, SOURCE_REMOVED, or REINDEXED",
        ));
    }
    Ok(())
}

fn validate_smart_cache_stale_response(
    expected_room_id: &str,
    expected_source_relative_path: Option<&str>,
    expected_reason: &str,
    response: SmartCacheStaleResponse,
) -> Result<AgentSmartCacheStaleResult, AgentError> {
    validate_smart_cache_stale_reason(&response.reason)
        .map_err(|_| invalid_response_error("smart cache stale response reason is invalid"))?;
    if response.room_id != expected_room_id
        || response.source_relative_path.as_deref() != expected_source_relative_path
        || response.reason != expected_reason
    {
        return Err(invalid_response_error(
            "smart cache stale response did not match the request",
        ));
    }
    if let Some(path) = &response.source_relative_path {
        validate_relative_path("smart cache stale response path", path)
            .map_err(|_| invalid_response_error("smart cache stale response path is invalid"))?;
    }
    Ok(AgentSmartCacheStaleResult {
        room_id: response.room_id,
        source_relative_path: response.source_relative_path,
        reason: response.reason,
        stale_count: response.stale_count,
    })
}

fn validate_smart_cache_encryption_metadata(
    metadata: &SmartCacheEncryptionMetadata,
) -> Result<(), AgentError> {
    if metadata.algorithm != crate::smart_cache_crypto::SMART_CACHE_ENCRYPTION_ALGORITHM
        || metadata.format != crate::smart_cache_crypto::SMART_CACHE_ENCRYPTION_FORMAT
        || metadata.key_id.trim().len() < 16
        || metadata.key_id.len() > 128
        || metadata.nonce_hex.len() != 24
        || !metadata
            .nonce_hex
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        || metadata.plaintext_size_bytes == 0
        || metadata.plaintext_sha256.len() != 64
        || !metadata
            .plaintext_sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(invalid_response_error(
            "smart cache encryption metadata failed response validation",
        ));
    }
    Ok(())
}

fn validate_cleanliness_snapshot(snapshot: &CleanlinessSnapshot) -> Result<(), AgentError> {
    let metrics = &snapshot.metrics;
    if snapshot.formula_version != crate::cleanliness::CLEANLINESS_FORMULA_VERSION
        || snapshot.score > 100
        || snapshot.calculated_at.trim().is_empty()
        || metrics.managed_file_count > metrics.total_file_count
        || metrics.unorganized_file_count > metrics.total_file_count
        || metrics.managed_file_count + metrics.unorganized_file_count > metrics.total_file_count
    {
        return Err(validation_error("cleanliness snapshot failed validation"));
    }

    for deduction in &metrics.deductions {
        if deduction.reason_code.trim().is_empty() || deduction.points > 100 {
            return Err(validation_error(
                "cleanliness snapshot deduction failed validation",
            ));
        }
    }

    Ok(())
}

fn validate_room_snapshot_response(
    expected_room_id: &str,
    expected_formula_version: &str,
    response: RoomSnapshotResponse,
) -> Result<AgentRoomSnapshot, AgentError> {
    let formula_version = response
        .formula_version
        .unwrap_or_else(|| expected_formula_version.to_string());
    if response.id.is_empty()
        || response.room_id != expected_room_id
        || formula_version != expected_formula_version
        || response.score > 100
        || !response.metrics.is_object()
        || response.calculated_at.trim().is_empty()
    {
        return Err(invalid_response_error(
            "room snapshot failed response validation",
        ));
    }
    Ok(AgentRoomSnapshot {
        snapshot_id: response.id,
        room_id: response.room_id,
        formula_version,
        score: response.score,
        metrics: response.metrics,
        calculated_at: response.calculated_at,
    })
}

fn validate_source_version(version: &AgentFileTransferSourceVersion) -> Result<(), AgentError> {
    if version.file_id.is_empty()
        || version.file_id.len() > 512
        || version.size_bytes == 0
        || version.modified_at.is_empty()
    {
        return Err(validation_error(
            "file transfer sourceVersion failed validation",
        ));
    }
    Ok(())
}

fn validate_relative_path(label: &str, path: &str) -> Result<(), AgentError> {
    if path.is_empty()
        || path.len() > 1024
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\0')
        || path.contains(':')
        || path
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(validation_error(&format!(
            "{label} must be a managed-root-relative path"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), AgentError> {
    if !is_valid_sha256(value) {
        return Err(validation_error(
            "file transfer sha256 must be 64 lowercase hex characters",
        ));
    }
    Ok(())
}

fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn validate_proposal_item(
    expected_proposal_id: &str,
    item: ProposalItemResponse,
) -> Result<AgentProposalItemRecord, AgentError> {
    if item.id.is_empty()
        || item.proposal_id != expected_proposal_id
        || item.reason_code.is_empty()
        || !matches!(
            item.action_type.as_str(),
            "MOVE" | "QUARANTINE" | "CREATE_DIR" | "CREATE_FILE" | "README_WRITE"
        )
        || !matches!(
            item.conflict_state.as_str(),
            "NONE" | "NAME_CONFLICT" | "UNSUPPORTED"
        )
    {
        return Err(invalid_response_error(
            "proposal item failed response validation",
        ));
    }
    Ok(AgentProposalItemRecord {
        item_order: item.item_order,
        action_type: item.action_type,
        source_relative_path: item.source_relative_path,
        destination_relative_path: item.destination_relative_path,
        reason_code: item.reason_code,
        precondition: item.precondition,
        conflict_state: item.conflict_state,
    })
}

fn validate_execution_response(response: ExecutionResponse) -> Result<AgentExecution, AgentError> {
    if response.id.is_empty()
        || !matches!(
            response.status.as_str(),
            "EXECUTING" | "SUCCEEDED" | "PARTIALLY_SUCCEEDED" | "FAILED" | "STALE" | "ROLLED_BACK"
        )
    {
        return Err(invalid_response_error(
            "execution failed response validation",
        ));
    }
    Ok(AgentExecution {
        execution_id: response.id,
        status: response.status,
    })
}

fn http_error(status: StatusCode, server_error: Option<ServerErrorResponse>) -> AgentError {
    let code = match status {
        StatusCode::UNAUTHORIZED => AgentErrorCode::Unauthenticated,
        StatusCode::FORBIDDEN => AgentErrorCode::Forbidden,
        _ => AgentErrorCode::TransportUnavailable,
    };
    let server_code = server_error.as_ref().and_then(|body| body.code.clone());
    let message = server_error
        .and_then(|body| body.message.or(body.code))
        .unwrap_or_else(|| format!("MouseKeeper server request failed with HTTP {status}"));
    AgentError {
        code,
        message,
        server_code,
    }
}

fn unconfigured_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::Unconfigured,
        message: message.to_string(),
        server_code: None,
    }
}

fn validation_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::ValidationFailed,
        message: message.to_string(),
        server_code: None,
    }
}

fn invalid_response_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::InvalidResponse,
        message: message.to_string(),
        server_code: None,
    }
}

fn credential_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::CredentialStoreUnavailable,
        message: message.to_string(),
        server_code: None,
    }
}

impl Clone for AgentError {
    fn clone(&self) -> Self {
        Self {
            code: self.code.clone(),
            message: self.message.clone(),
            server_code: self.server_code.clone(),
        }
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl AgentErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentErrorCode::Unconfigured => "UNCONFIGURED",
            AgentErrorCode::ValidationFailed => "VALIDATION_FAILED",
            AgentErrorCode::TransportUnavailable => "TRANSPORT_UNAVAILABLE",
            AgentErrorCode::Unauthenticated => "UNAUTHENTICATED",
            AgentErrorCode::Forbidden => "FORBIDDEN",
            AgentErrorCode::InvalidResponse => "INVALID_RESPONSE",
            AgentErrorCode::CredentialStoreUnavailable => "CREDENTIAL_STORE_UNAVAILABLE",
        }
    }
}

impl AgentError {
    /// True when the failure is worth retrying later (the request never reached a definitive
    /// server verdict). A transport failure — server unreachable, timed out — is transient. A
    /// validation/authorization/response error is a definitive rejection and must not be retried
    /// forever with the same payload.
    pub fn is_transient(&self) -> bool {
        matches!(self.code, AgentErrorCode::TransportUnavailable)
    }

    pub fn is_confirmed_device_absent(&self) -> bool {
        matches!(
            self.server_code.as_deref(),
            Some("DEVICE_REVOKED" | "DEVICE_NOT_REGISTERED")
        )
    }
}

impl std::error::Error for AgentError {}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::{
        normalize_server_base_url, AgentConnectionState, AgentError, AgentErrorCode,
        AgentFileSearchScope, AgentRuntime, CredentialStore, DeviceCredential, ServerCommand,
    };

    #[derive(Default)]
    struct MemoryCredentialStore {
        credential: Mutex<Option<DeviceCredential>>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load(&self) -> Result<Option<DeviceCredential>, AgentError> {
            Ok(self.credential.lock().expect("memory store").clone())
        }

        fn save(&self, credential: &DeviceCredential) -> Result<(), AgentError> {
            *self.credential.lock().expect("memory store") = Some(credential.clone());
            Ok(())
        }

        fn delete(&self) -> Result<(), AgentError> {
            *self.credential.lock().expect("memory store") = None;
            Ok(())
        }
    }

    #[test]
    fn missing_server_url_is_explicitly_unconfigured() {
        let runtime = AgentRuntime::for_test(None, Arc::new(MemoryCredentialStore::default()));
        let status = runtime.connection_status();

        assert_eq!(status.state, AgentConnectionState::Unconfigured);
        assert_eq!(status.last_error_code, Some(AgentErrorCode::Unconfigured));
    }

    #[test]
    fn paired_device_starts_offline_until_a_request_succeeds() {
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: "http://127.0.0.1:3000".to_string(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some("http://127.0.0.1:3000"), Arc::new(store));
        let status = runtime.connection_status();

        assert_eq!(status.state, AgentConnectionState::Offline);
        assert_eq!(status.device_id.as_deref(), Some("device-1"));
    }

    #[test]
    fn credential_token_is_never_exposed_in_connection_status() {
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: "https://example.com".to_string(),
                device_id: "device-1".to_string(),
                device_token: "must-not-leak".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some("https://example.com"), Arc::new(store));

        let serialized = serde_json::to_string(&runtime.connection_status()).expect("status json");

        assert!(!serialized.contains("must-not-leak"));
    }

    #[test]
    fn realtime_credentials_require_pairing() {
        let unpaired = AgentRuntime::for_test(
            Some("http://127.0.0.1:3000"),
            Arc::new(MemoryCredentialStore::default()),
        );
        assert!(unpaired.realtime_credentials().is_none());

        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: "http://127.0.0.1:3000".to_string(),
                device_id: "device-1".to_string(),
                device_token: "mk_device_secret".to_string(),
            })),
        };
        let paired = AgentRuntime::for_test(Some("http://127.0.0.1:3000"), Arc::new(store));
        let credentials = paired
            .realtime_credentials()
            .expect("paired runtime exposes realtime credentials");
        assert_eq!(credentials.base_url, "http://127.0.0.1:3000");
        assert_eq!(credentials.device_token, "mk_device_secret");
    }

    #[test]
    fn server_url_accepts_origin_and_rejects_unsafe_parts() {
        assert_eq!(
            normalize_server_base_url(" http://127.0.0.1:3000/ ").expect("valid URL"),
            "http://127.0.0.1:3000"
        );
        assert!(normalize_server_base_url("file:///tmp/server").is_err());
        assert!(normalize_server_base_url("https://user:pass@example.com").is_err());
        assert!(normalize_server_base_url("https://example.com/api").is_err());
        assert!(normalize_server_base_url("http://example.com").is_err());
    }

    #[test]
    fn server_command_maps_contract_fields_without_private_idempotency_key() {
        let raw = r#"{
            "id":"command-1",
            "intent":"ORGANIZE_ROOT",
            "roomId":"room-1",
            "status":"QUEUED",
            "payload":{"rootId":"root-1"},
            "createdAt":"2026-07-12T00:00:00.000Z"
        }"#;
        let command: ServerCommand = serde_json::from_str(raw).expect("server command");
        let command: super::AgentCommand = command.into();

        assert_eq!(command.command_id, "command-1");
        assert_eq!(command.command_type, "ORGANIZE_ROOT");
        assert_eq!(command.status, "QUEUED");
    }

    #[test]
    fn heartbeat_rejects_offline_presence_before_transport() {
        let runtime = AgentRuntime::for_test(
            Some("http://127.0.0.1:3000"),
            Arc::new(MemoryCredentialStore::default()),
        );
        let error = tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(runtime.heartbeat("OFFLINE".to_string()))
            .expect_err("OFFLINE must be rejected");

        assert_eq!(error.code, AgentErrorCode::ValidationFailed);
    }

    #[tokio::test]
    async fn pairing_session_uses_the_server_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/pairing-sessions",
            r#"{"sessionId":"session-1","desktopNonce":"nonce-1","code":"123456","expiresAt":"2026-07-12T01:00:00.000Z"}"#,
            |request| {
                assert!(request.starts_with("POST /v1/pairing-sessions HTTP/1.1"));
                assert!(request.contains(r#""deviceName":"Test Desktop""#));
                assert!(request.contains(r#""platform":"WINDOWS""#));
            },
        );
        let runtime = AgentRuntime::for_test(
            Some(&server_url),
            Arc::new(MemoryCredentialStore::default()),
        );

        let session = runtime
            .start_pairing("Test Desktop".to_string())
            .await
            .expect("pairing session");
        server.join().expect("server thread");

        assert_eq!(session.code, "123456");
        assert_eq!(session.desktop_nonce, "nonce-1");
        assert_eq!(
            runtime.connection_status().state,
            AgentConnectionState::Connecting
        );
    }

    #[tokio::test]
    async fn claimed_pairing_is_saved_without_returning_the_token() {
        let (server_url, server) = one_shot_json_server(
            "/v1/pairing-sessions/session-1/status?nonce=nonce-1",
            r#"{"status":"CLAIMED","deviceId":"device-1","deviceToken":"mk_device_secret"}"#,
            |request| {
                assert!(request.starts_with(
                    "GET /v1/pairing-sessions/session-1/status?nonce=nonce-1 HTTP/1.1"
                ));
            },
        );
        let store = Arc::new(MemoryCredentialStore::default());
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let status = runtime
            .poll_pairing("session-1".to_string(), "nonce-1".to_string())
            .await
            .expect("claimed status");
        server.join().expect("server thread");

        assert_eq!(status.device_id.as_deref(), Some("device-1"));
        assert!(!serde_json::to_string(&status)
            .expect("status json")
            .contains("mk_device_secret"));
        assert_eq!(
            store
                .load()
                .expect("credential")
                .expect("saved")
                .device_token,
            "mk_device_secret"
        );
    }

    #[tokio::test]
    async fn heartbeat_uses_the_control_plane_presence_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/devices/device-1/heartbeat",
            r#"{"deviceId":"device-1","presence":"ONLINE_IDLE","ttlSeconds":45}"#,
            |request| {
                assert!(request.starts_with("POST /v1/devices/device-1/heartbeat HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
                assert!(request.contains(r#""presence":"ONLINE_IDLE""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let heartbeat = runtime
            .heartbeat("ONLINE_IDLE".to_string())
            .await
            .expect("heartbeat");
        server.join().expect("server thread");

        assert_eq!(heartbeat.presence, "ONLINE_IDLE");
        assert_eq!(
            runtime.connection_status().state,
            AgentConnectionState::Online
        );
    }

    #[tokio::test]
    async fn self_revoke_uses_delete_and_keeps_credential_until_common_cleanup() {
        let (server_url, server) = one_shot_json_server("/v1/agent/devices/self", "", |request| {
            assert!(request.starts_with("DELETE /v1/agent/devices/self HTTP/1.1"));
            let lower = request.to_ascii_lowercase();
            assert!(lower.contains("authorization: bearer secret-token"));
            assert!(lower.contains("idempotency-key: revoke-request-1"));
        });
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        runtime
            .revoke_self("revoke-request-1".to_string())
            .await
            .expect("revoke accepted");
        server.join().expect("server thread");

        assert!(store.load().expect("credential store").is_some());
        assert_eq!(
            runtime.connection_status().device_id.as_deref(),
            Some("device-1")
        );
    }

    #[tokio::test]
    async fn self_revoke_transport_failure_preserves_retry_credential() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused port");
        let server_url = format!("http://{}", listener.local_addr().expect("address"));
        drop(listener);
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let error = runtime
            .revoke_self("same-retry-key".to_string())
            .await
            .expect_err("transport failure");

        assert_eq!(error.code, AgentErrorCode::TransportUnavailable);
        assert!(store.load().expect("credential store").is_some());
        assert_eq!(
            runtime.connection_status().device_id.as_deref(),
            Some("device-1")
        );
    }

    #[tokio::test]
    async fn self_revoke_auth_rejection_is_not_reported_as_success() {
        let (server_url, server) = one_shot_error_server(
            "/v1/agent/devices/self",
            "HTTP/1.1 403 Forbidden",
            r#"{"code":"FORBIDDEN","message":"request was not authorized"}"#,
            |request| {
                assert!(request.starts_with("DELETE /v1/agent/devices/self HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("idempotency-key: same-retry-key"));
            },
        );
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let error = runtime
            .revoke_self("same-retry-key".to_string())
            .await
            .expect_err("an auth rejection cannot prove the revoke transaction committed");
        server.join().expect("server thread");

        assert_eq!(error.code, AgentErrorCode::Forbidden);
        assert!(store.load().expect("credential store").is_some());
        let status = runtime.connection_status();
        assert_eq!(status.state, AgentConnectionState::Offline);
        assert_eq!(status.device_id.as_deref(), Some("device-1"));
    }

    #[tokio::test]
    async fn generic_unauthenticated_heartbeat_does_not_claim_device_revocation() {
        let (server_url, server) = one_shot_error_server(
            "/v1/devices/device-1/heartbeat",
            "HTTP/1.1 401 Unauthorized",
            r#"{"code":"UNAUTHENTICATED","message":"token was not accepted"}"#,
            |_| {},
        );
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let error = runtime
            .heartbeat("ONLINE_IDLE".to_string())
            .await
            .expect_err("unauthenticated response");
        server.join().expect("server thread");

        assert_eq!(error.code, AgentErrorCode::Unauthenticated);
        assert!(!error.is_confirmed_device_absent());
        assert!(store.load().expect("credential store").is_some());
        assert_eq!(
            runtime.connection_status().device_id.as_deref(),
            Some("device-1")
        );
    }

    #[tokio::test]
    async fn auth_failure_revokes_local_pairing_and_realtime_credentials() {
        let (server_url, server) = one_shot_error_server(
            "/v1/devices/device-1/heartbeat",
            "HTTP/1.1 403 Forbidden",
            r#"{"code":"DEVICE_REVOKED","message":"device revoked"}"#,
            |request| {
                assert!(request.starts_with("POST /v1/devices/device-1/heartbeat HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
            },
        );
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let error = runtime
            .heartbeat("ONLINE_IDLE".to_string())
            .await
            .expect_err("revoked devices must fail heartbeat");
        server.join().expect("server thread");

        let status = runtime.connection_status();
        assert_eq!(error.code, AgentErrorCode::Forbidden);
        assert_eq!(status.state, AgentConnectionState::Revoked);
        assert_eq!(status.device_id, None);
        assert_eq!(status.last_error_code, Some(AgentErrorCode::Forbidden));
        assert_eq!(status.last_error_message.as_deref(), Some("device revoked"));
        assert!(runtime.realtime_credentials().is_none());
        assert!(store.load().expect("credential store").is_none());
    }

    #[tokio::test]
    async fn missing_server_device_starts_fresh_pairing_state() {
        let (server_url, server) = one_shot_error_server(
            "/v1/devices/device-1/heartbeat",
            "HTTP/1.1 401 Unauthorized",
            r#"{"code":"DEVICE_NOT_REGISTERED","message":"pairing no longer exists"}"#,
            |_| {},
        );
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let error = runtime
            .heartbeat("ONLINE_IDLE".to_string())
            .await
            .expect_err("a removed device row must fail heartbeat");
        server.join().expect("server thread");

        assert!(error.is_confirmed_device_absent());
        assert_eq!(
            runtime.connection_status().state,
            AgentConnectionState::Revoked
        );
        assert_eq!(runtime.connection_status().device_id, None);
        assert!(runtime.realtime_credentials().is_none());
        assert!(store.load().expect("credential store").is_none());
    }

    #[tokio::test]
    async fn forbidden_heartbeat_stays_offline_without_dropping_pairing() {
        // A 403 is a per-endpoint authorization decision, not a pairing revocation, so it must not
        // delete local credentials or force a re-pair — the device simply goes Offline and retries.
        let (server_url, server) = one_shot_error_server(
            "/v1/devices/device-1/heartbeat",
            "HTTP/1.1 403 Forbidden",
            r#"{"code":"FORBIDDEN","message":"forbidden"}"#,
            |request| {
                assert!(request.starts_with("POST /v1/devices/device-1/heartbeat HTTP/1.1"));
            },
        );
        let store = Arc::new(MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        });
        let runtime = AgentRuntime::for_test(Some(&server_url), store.clone());

        let error = runtime
            .heartbeat("ONLINE_IDLE".to_string())
            .await
            .expect_err("forbidden heartbeat still fails");
        server.join().expect("server thread");

        let status = runtime.connection_status();
        assert_eq!(error.code, AgentErrorCode::Forbidden);
        assert_eq!(status.state, AgentConnectionState::Offline);
        // The pairing survives: device id, saved credential, and realtime credential all remain.
        assert_eq!(status.device_id.as_deref(), Some("device-1"));
        assert!(runtime.realtime_credentials().is_some());
        assert!(store.load().expect("credential store").is_some());
    }

    #[tokio::test]
    async fn managed_root_creates_a_mobile_room_through_the_server_contract() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind room server");
        let address = listener.local_addr().expect("room server address");
        let server_url = format!("http://{address}");
        let server = thread::spawn(move || {
            let expectations = [
                ("GET /v1/rooms HTTP/1.1", "[]"),
                (
                    "POST /v1/rooms HTTP/1.1",
                    r#"{"id":"room-1","desktopDeviceId":"device-1","name":"Downloads","rootAlias":"root:abc123","status":"ACTIVE"}"#,
                ),
            ];
            for (expected_request, response_body) in expectations {
                let (mut stream, _) = listener.accept().expect("accept room request");
                let mut buffer = [0_u8; 8192];
                let length = stream.read(&mut buffer).expect("read room request");
                let request = String::from_utf8_lossy(&buffer[..length]);
                assert!(request.starts_with(expected_request));
                if expected_request.starts_with("POST") {
                    assert!(request.contains(r#""desktopDeviceId":"device-1""#));
                    assert!(request.contains(r#""rootAlias":"root:abc123""#));
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write room response");
            }
        });
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let room = runtime
            .ensure_room_for_root("root:abc123".to_string(), "Downloads".to_string())
            .await
            .expect("room sync");
        server.join().expect("room server thread");

        assert_eq!(room.room_id, "room-1");
        assert!(room.created);
    }

    #[tokio::test]
    async fn pending_decisions_maps_the_control_plane_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/devices/device-1/decisions/pending",
            r#"[{
                "decision":{"id":"decision-1","proposalId":"proposal-1","decisionType":"APPROVE","approvedItemIds":["item-1"]},
                "proposal":{"id":"proposal-1","commandId":"command-1","roomId":"room-1","status":"APPROVED"},
                "items":[{
                    "id":"item-1",
                    "proposalId":"proposal-1",
                    "itemOrder":0,
                    "actionType":"MOVE",
                    "sourceRelativePath":"a.pdf",
                    "destinationRelativePath":"Documents/a.pdf",
                    "reasonCode":"RULE_MOVE_BY_EXTENSION",
                    "precondition":{"sourceSizeBytes":10,"sourceModifiedUnixMs":123},
                    "conflictState":"NONE"
                }]
            }]"#,
            |request| {
                assert!(request.starts_with("GET /v1/devices/device-1/decisions/pending HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let decisions = runtime
            .pending_decisions()
            .await
            .expect("pending decisions");
        server.join().expect("server thread");

        assert_eq!(decisions.len(), 1);
        let decision = &decisions[0];
        assert_eq!(decision.decision_id, "decision-1");
        assert_eq!(decision.proposal_id, "proposal-1");
        assert_eq!(decision.room_id, "room-1");
        assert_eq!(decision.items.len(), 1);
        assert_eq!(decision.items[0].action_type, "MOVE");
        assert_eq!(
            decision.items[0].source_relative_path.as_deref(),
            Some("a.pdf")
        );
    }

    #[tokio::test]
    async fn pending_file_browse_maps_search_query_and_scope() {
        let (server_url, server) = one_shot_json_server(
            "/v1/devices/device-1/file-browse-requests/pending",
            r#"[{
                "id":"browse-1",
                "roomId":"room-1",
                "relativeDirectory":"docs",
                "cursor":null,
                "query":"report",
                "extensions":[".pdf"],
                "limit":25,
                "searchScope":"MANAGED_ROOT",
                "status":"REQUESTED"
            }]"#,
            |request| {
                assert!(request
                    .starts_with("GET /v1/devices/device-1/file-browse-requests/pending HTTP/1.1"));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let requests = runtime
            .pending_file_browse_requests()
            .await
            .expect("pending browse");
        server.join().expect("server thread");

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].query.as_deref(), Some("report"));
        assert_eq!(requests[0].extensions, vec![".pdf"]);
        assert_eq!(requests[0].limit, 25);
        assert_eq!(requests[0].search_scope, AgentFileSearchScope::ManagedRoot);
    }

    #[test]
    fn file_browse_query_limit_counts_unicode_characters_not_utf8_bytes() {
        let query = "가".repeat(34);
        assert!(query.len() > 100);
        let request = super::validate_file_browse_request(super::FileBrowseRequestResponse {
            id: "browse-1".to_string(),
            room_id: "room-1".to_string(),
            relative_directory: String::new(),
            cursor: None,
            query: Some(query.clone()),
            extensions: Vec::new(),
            limit: 200,
            search_scope: AgentFileSearchScope::ManagedRoot,
            status: "REQUESTED".to_string(),
        })
        .expect("34 Korean characters are within the 100-character contract limit");

        assert_eq!(request.query.as_deref(), Some(query.as_str()));
    }

    #[tokio::test]
    async fn pending_file_transfers_maps_the_control_plane_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/devices/device-1/file-transfers/pending",
            r#"[{
                "id":"transfer-1",
                "roomId":"room-1",
                "desktopDeviceId":"device-1",
                "sourceRelativePath":"docs/report.pdf",
                "sourceVersion":null,
                "status":"REQUESTED",
                "failureCode":null,
                "sizeBytes":null,
                "sha256":null,
                "expiresAt":"2026-07-13T01:00:00.000Z",
                "completedAt":null,
                "createdAt":"2026-07-13T00:00:00.000Z"
            }]"#,
            |request| {
                assert!(
                    request.starts_with("GET /v1/devices/device-1/file-transfers/pending HTTP/1.1")
                );
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let transfers = runtime
            .pending_file_transfers()
            .await
            .expect("pending transfers");
        server.join().expect("server thread");

        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].transfer_id, "transfer-1");
        assert_eq!(transfers[0].source_relative_path, "docs/report.pdf");
        assert_eq!(transfers[0].status, "REQUESTED");
    }

    #[tokio::test]
    async fn upload_target_uses_validated_source_version_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/agent/file-transfers/transfer-1/upload-target",
            r#"{"transferId":"transfer-1","uploadUrl":"http://127.0.0.1:9000/bucket/object?signature=ok","expiresAt":"2026-07-13T01:00:00.000Z"}"#,
            |request| {
                assert!(request.starts_with(
                    "POST /v1/agent/file-transfers/transfer-1/upload-target HTTP/1.1"
                ));
                assert!(request.contains(r#""fileId":"hm:abc""#));
                assert!(request.contains(r#""sizeBytes":42"#));
                assert!(request.contains(r#""modifiedAt":"2026-07-13T00:00:00.000Z""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let target = runtime
            .request_file_transfer_upload_target(
                "transfer-1".to_string(),
                super::AgentFileTransferSourceVersion {
                    file_id: "hm:abc".to_string(),
                    size_bytes: 42,
                    modified_at: "2026-07-13T00:00:00.000Z".to_string(),
                },
            )
            .await
            .expect("upload target");
        server.join().expect("server thread");

        assert_eq!(target.transfer_id, "transfer-1");
        assert!(target.upload_url.starts_with("http://127.0.0.1:9000/"));
    }

    #[tokio::test]
    async fn create_execution_claims_the_decision_with_an_idempotency_key() {
        let (server_url, server) = one_shot_json_server(
            "/v1/agent/executions",
            r#"{"id":"execution-1","status":"EXECUTING"}"#,
            |request| {
                assert!(request.starts_with("POST /v1/agent/executions HTTP/1.1"));
                assert!(request.contains("idempotency-key: decision-1"));
                assert!(request.contains(r#""proposalId":"proposal-1""#));
                assert!(request.contains(r#""decisionId":"decision-1""#));
                assert!(request.contains(r#""desktopDeviceId":"device-1""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let execution = runtime
            .create_execution("proposal-1".to_string(), "decision-1".to_string())
            .await
            .expect("execution");
        server.join().expect("server thread");

        assert_eq!(execution.execution_id, "execution-1");
        assert_eq!(execution.status, "EXECUTING");
    }

    #[tokio::test]
    async fn update_execution_uploads_the_terminal_result() {
        let (server_url, server) = one_shot_json_server(
            "/v1/agent/executions/execution-1",
            r#"{"id":"execution-1","status":"SUCCEEDED"}"#,
            |request| {
                assert!(request.starts_with("PATCH /v1/agent/executions/execution-1 HTTP/1.1"));
                assert!(request.contains("idempotency-key: execution-1"));
                assert!(request.contains(r#""status":"SUCCEEDED""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let execution = runtime
            .update_execution(
                "execution-1".to_string(),
                "SUCCEEDED".to_string(),
                serde_json::json!({ "executedCount": 1 }),
            )
            .await
            .expect("execution");
        server.join().expect("server thread");

        assert_eq!(execution.status, "SUCCEEDED");
    }

    #[tokio::test]
    async fn room_snapshot_uses_the_cleanliness_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/rooms/room-1/snapshots",
            r#"{"id":"snapshot-1","roomId":"room-1","score":88,"metrics":{"totalFileCount":10,"managedFileCount":8,"unorganizedFileCount":2,"deductions":[]},"calculatedAt":"2026-07-13T00:00:00.000Z"}"#,
            |request| {
                assert!(request.starts_with("POST /v1/rooms/room-1/snapshots HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
                assert!(request.contains(r#""formulaVersion":"mousekeeper-cleanliness-v1""#));
                assert!(request.contains(r#""score":88"#));
                assert!(request.contains(r#""totalFileCount":10"#));
                assert!(request.contains(r#""calculatedAt":"2026-07-13T00:00:00.000Z""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));
        let snapshot = crate::cleanliness::CleanlinessSnapshot {
            formula_version: crate::cleanliness::CLEANLINESS_FORMULA_VERSION.to_string(),
            score: 88,
            metrics: crate::cleanliness::CleanlinessMetrics {
                total_file_count: 10,
                managed_file_count: 8,
                unorganized_file_count: 2,
                deductions: Vec::new(),
            },
            calculated_at: "2026-07-13T00:00:00.000Z".to_string(),
        };

        let saved = runtime
            .submit_room_snapshot("room-1".to_string(), snapshot)
            .await
            .expect("room snapshot");
        server.join().expect("server thread");

        assert_eq!(saved.snapshot_id, "snapshot-1");
        assert_eq!(saved.room_id, "room-1");
        assert_eq!(
            saved.formula_version,
            crate::cleanliness::CLEANLINESS_FORMULA_VERSION
        );
        assert_eq!(saved.score, 88);
    }

    #[tokio::test]
    async fn smart_cache_stale_report_uses_the_agent_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/agent/cached-files/stale",
            r#"{"roomId":"room-1","sourceRelativePath":"docs/a.pdf","reason":"SOURCE_CHANGED","staleCount":1}"#,
            |request| {
                assert!(request.starts_with("POST /v1/agent/cached-files/stale HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
                assert!(request.contains("idempotency-key: stale-1"));
                assert!(request.contains(r#""roomId":"room-1""#));
                assert!(request.contains(r#""sourceRelativePath":"docs/a.pdf""#));
                assert!(request.contains(r#""reason":"SOURCE_CHANGED""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let result = runtime
            .mark_smart_cache_stale(
                "stale-1".to_string(),
                "room-1".to_string(),
                Some("docs/a.pdf".to_string()),
                "SOURCE_CHANGED".to_string(),
            )
            .await
            .expect("stale report");
        server.join().expect("server thread");

        assert_eq!(result.room_id, "room-1");
        assert_eq!(result.source_relative_path.as_deref(), Some("docs/a.pdf"));
        assert_eq!(result.reason, "SOURCE_CHANGED");
        assert_eq!(result.stale_count, 1);
    }

    #[tokio::test]
    async fn desktop_chat_messages_use_the_shared_server_session_contract() {
        let (server_url, server) = one_shot_json_server(
            "/v1/chat-sessions/session-1/messages",
            r#"{"message":{"id":"message-1","roomId":"room-1","sessionId":"session-1","senderType":"USER","messageType":"TEXT","content":"정리해줘","structuredPayload":{},"commandId":null,"createdAt":"2026-07-14T00:00:00.000Z"},"assistant":null,"aiStatus":"UNCONFIGURED","ai":{"status":"UNCONFIGURED","code":"AI_PROVIDER_UNCONFIGURED"}}"#,
            |request| {
                assert!(request.starts_with("POST /v1/chat-sessions/session-1/messages HTTP/1.1"));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-token"));
                assert!(request.contains(r#""content":"정리해줘""#));
            },
        );
        let store = MemoryCredentialStore {
            credential: Mutex::new(Some(DeviceCredential {
                server_base_url: server_url.clone(),
                device_id: "device-1".to_string(),
                device_token: "secret-token".to_string(),
            })),
        };
        let runtime = AgentRuntime::for_test(Some(&server_url), Arc::new(store));

        let result = runtime
            .send_chat_message("session-1".to_string(), "정리해줘".to_string())
            .await
            .expect("chat send");
        server.join().expect("server thread");

        assert_eq!(result.message.message_id, "message-1");
        assert_eq!(result.message.session_id.as_deref(), Some("session-1"));
        assert_eq!(result.ai_status, "UNCONFIGURED");
        assert!(result.assistant.is_none());
    }

    #[tokio::test]
    async fn update_execution_rejects_a_non_object_result_summary() {
        let runtime = AgentRuntime::for_test(
            Some("http://127.0.0.1:3000"),
            Arc::new(MemoryCredentialStore::default()),
        );

        let error = runtime
            .update_execution(
                "execution-1".to_string(),
                "SUCCEEDED".to_string(),
                serde_json::json!([1, 2, 3]),
            )
            .await
            .expect_err("array result summary must be rejected");

        assert_eq!(error.code, AgentErrorCode::ValidationFailed);
    }

    fn one_shot_json_server<F>(
        expected_path: &'static str,
        response_body: &'static str,
        inspect: F,
    ) -> (String, thread::JoinHandle<()>)
    where
        F: FnOnce(&str) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 8192];
            let length = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..length]);
            assert!(request.contains(expected_path));
            inspect(&request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        (format!("http://{address}"), handle)
    }

    fn one_shot_error_server<F>(
        expected_path: &'static str,
        status_line: &'static str,
        response_body: &'static str,
        inspect: F,
    ) -> (String, thread::JoinHandle<()>)
    where
        F: FnOnce(&str) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 8192];
            let length = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..length]);
            assert!(request.contains(expected_path));
            inspect(&request);
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        (format!("http://{address}"), handle)
    }
}
