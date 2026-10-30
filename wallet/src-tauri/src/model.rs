use crypto_sso_identity_api::{EnrollmentView, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct Center {
    pub id: String,
    pub base_url: String,
    pub info: ServiceInfo,
}

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct Draft {
    pub id: String,
    pub center_id: String,
    pub rows: Vec<(String, String)>,
    pub claim_hints_revision: Option<u32>,
    pub application_note: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalEnrollment {
    pub id: String,
    pub center_id: String,
    pub pending: Option<Zeroizing<String>>,
    pub access: Zeroizing<String>,
    pub submission: String,
    pub claims: BTreeMap<String, String>,
    pub view: Option<EnrollmentView>,
    pub local_state: String,
    pub response: Option<String>,
    pub error: Option<String>,
    pub synced_at: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub center_id: String,
    pub issuer_id: String,
    pub claims: BTreeMap<String, String>,
    pub saved_at: u64,
    pub holder: Zeroizing<String>,
}

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct TrustedApp {
    pub id: String,
    pub name: String,
    pub origin: String,
    pub issuer_id: String,
    pub revision: u32,
}

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct LoginTask {
    pub id: String,
    pub app_id: String,
    pub app_revision: u32,
    pub revision: u32,
    pub challenge: String,
    pub state: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub version: u8,
    pub centers: BTreeMap<String, Center>,
    pub drafts: BTreeMap<String, Draft>,
    pub enrollments: BTreeMap<String, LocalEnrollment>,
    pub credentials: BTreeMap<String, Credential>,
    pub apps: BTreeMap<String, TrustedApp>,
    pub tasks: BTreeMap<String, LoginTask>,
    pub auto_lock_seconds: u32,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: 1,
            centers: BTreeMap::new(),
            drafts: BTreeMap::new(),
            enrollments: BTreeMap::new(),
            credentials: BTreeMap::new(),
            apps: BTreeMap::new(),
            tasks: BTreeMap::new(),
            auto_lock_seconds: 300,
        }
    }
}
