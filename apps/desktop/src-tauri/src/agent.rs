use std::env;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::command_processor::AgentProposalSubmission;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SERVER_BASE_URL_ENV: &str = "HOUSEMOUSE_SERVER_BASE_URL";
const KEYRING_SERVICE: &str = "com.housemouse.desktop";
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
            message: "cannot reach the HouseMouse server".to_string(),
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
            message: "cannot reach the HouseMouse server".to_string(),
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
    status: String,
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
    rooms
        .into_iter()
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
    })
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
            "MOVE" | "QUARANTINE" | "CREATE_DIR" | "README_WRITE"
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
    let message = server_error
        .and_then(|body| body.message.or(body.code))
        .unwrap_or_else(|| format!("HouseMouse server request failed with HTTP {status}"));
    AgentError { code, message }
}

fn unconfigured_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::Unconfigured,
        message: message.to_string(),
    }
}

fn validation_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::ValidationFailed,
        message: message.to_string(),
    }
}

fn invalid_response_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::InvalidResponse,
        message: message.to_string(),
    }
}

fn credential_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::CredentialStoreUnavailable,
        message: message.to_string(),
    }
}

impl Clone for AgentError {
    fn clone(&self) -> Self {
        Self {
            code: self.code.clone(),
            message: self.message.clone(),
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
}

impl std::error::Error for AgentError {}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::{
        normalize_server_base_url, AgentConnectionState, AgentError, AgentErrorCode, AgentRuntime,
        CredentialStore, DeviceCredential, ServerCommand,
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
                device_token: "hm_device_secret".to_string(),
            })),
        };
        let paired = AgentRuntime::for_test(Some("http://127.0.0.1:3000"), Arc::new(store));
        let credentials = paired
            .realtime_credentials()
            .expect("paired runtime exposes realtime credentials");
        assert_eq!(credentials.base_url, "http://127.0.0.1:3000");
        assert_eq!(credentials.device_token, "hm_device_secret");
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
            r#"{"status":"CLAIMED","deviceId":"device-1","deviceToken":"hm_device_secret"}"#,
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
            .contains("hm_device_secret"));
        assert_eq!(
            store
                .load()
                .expect("credential")
                .expect("saved")
                .device_token,
            "hm_device_secret"
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
}
