use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use urlencoding::encode;
use uuid::Uuid;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_JQL: &str =
    "assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC";
const DEFAULT_TEAMS_OAUTH_CLIENT_ID: &str = "8ec6bc83-69c8-4392-8f08-b3c986009232";
const DEFAULT_TEAMS_DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/devicecode";
const DEFAULT_TEAMS_V1_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/token";
const DEFAULT_TEAMS_V2_TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const DEFAULT_TEAMS_CONSUMER_AUTH_URL: &str = "https://teams.live.com/api/auth/v1.0/authz/consumer";
const DEFAULT_TEAMS_CHAT_URL: &str = "https://msgapi.teams.live.com";
const DEFAULT_TEAMS_CHAT_AFD_URL: &str = "https://teams.live.com/api/chatsvc/consumer";
const TEAMS_OAUTH_RESOURCE: &str = "https://api.spaces.skype.com";
const TEAMS_SKYPE_SCOPE: &str =
    "service::api.fl.spaces.skype.com::MBI_SSL openid profile offline_access";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Watch,
    Once,
    Login,
    Logout,
    ListChats,
    TestTeams,
    Help,
    Version,
}

#[derive(Debug)]
struct AppConfig {
    jira_url: String,
    jira_email: String,
    jira_api_token: String,
    jql: String,
    poll_interval: u64,
    state_file: PathBuf,
    max_results: u32,
}

impl AppConfig {
    fn from_env() -> Result<Self> {
        let jira_url = required_env("JIRA_URL")?.trim_end_matches('/').to_string();
        let jira_email = required_env("JIRA_EMAIL")?;
        let jira_api_token = required_env("JIRA_API_TOKEN")?;
        let jql = env::var("JQL").unwrap_or_else(|_| DEFAULT_JQL.to_string());
        let poll_interval = parse_env("POLL_INTERVAL", 60_u64)?;
        let max_results = parse_env("MAX_RESULTS", 50_u32)?;
        let state_file = match optional_nonempty_env("STATE_FILE") {
            Some(path) => PathBuf::from(path),
            None => home_dir()?.join(".cache/jira2teams/state.json"),
        };

        Ok(Self {
            jira_url,
            jira_email,
            jira_api_token,
            jql,
            poll_interval,
            state_file,
            max_results,
        })
    }
}

#[derive(Debug, Clone)]
struct TeamsConfig {
    thread_id: Option<String>,
    target_name: Option<String>,
    auth_file: PathBuf,
    oauth_client_id: String,
    device_code_url: String,
    v1_token_url: String,
    v2_token_url: String,
    consumer_auth_url: String,
    chat_url: String,
    chat_afd_url: String,
}

impl TeamsConfig {
    fn from_env(require_target: bool) -> Result<Self> {
        let thread_id = optional_nonempty_env("TEAMS_THREAD_ID");
        let target_name = optional_nonempty_env("TEAMS_TO");

        if require_target && thread_id.is_none() && target_name.is_none() {
            bail!("Nastav TEAMS_THREAD_ID nebo TEAMS_TO. TEAMS_THREAD_ID je spolehlivejsi; zjistis ho pomoci --list-chats.");
        }

        let auth_file = match optional_nonempty_env("TEAMS_AUTH_FILE") {
            Some(path) => PathBuf::from(path),
            None => xdg_config_dir()?.join("jira2teams/teams-auth.json"),
        };

        Ok(Self {
            thread_id,
            target_name,
            auth_file,
            oauth_client_id: env::var("TEAMS_OAUTH_CLIENT_ID")
                .unwrap_or_else(|_| DEFAULT_TEAMS_OAUTH_CLIENT_ID.to_string()),
            device_code_url: env::var("TEAMS_DEVICE_CODE_URL")
                .unwrap_or_else(|_| DEFAULT_TEAMS_DEVICE_CODE_URL.to_string()),
            v1_token_url: env::var("TEAMS_V1_TOKEN_URL")
                .unwrap_or_else(|_| DEFAULT_TEAMS_V1_TOKEN_URL.to_string()),
            v2_token_url: env::var("TEAMS_V2_TOKEN_URL")
                .unwrap_or_else(|_| DEFAULT_TEAMS_V2_TOKEN_URL.to_string()),
            consumer_auth_url: env::var("TEAMS_AUTH_URL")
                .unwrap_or_else(|_| DEFAULT_TEAMS_CONSUMER_AUTH_URL.to_string()),
            chat_url: env::var("TEAMS_CHAT_URL")
                .unwrap_or_else(|_| DEFAULT_TEAMS_CHAT_URL.to_string()),
            chat_afd_url: env::var("TEAMS_CHAT_AFD_URL")
                .unwrap_or_else(|_| DEFAULT_TEAMS_CHAT_AFD_URL.to_string()),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TeamsAuthCache {
    version: u32,
    refresh_token: String,
    saved_unix: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    #[serde(alias = "verification_uri")]
    verification_url: String,
    #[serde(deserialize_with = "deserialize_u64_or_string")]
    expires_in: u64,
    #[serde(default, deserialize_with = "deserialize_optional_u64_or_string")]
    interval: Option<u64>,
    message: Option<String>,
}

fn deserialize_u64_or_string<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    value_to_u64(value).map_err(serde::de::Error::custom)
}

fn deserialize_optional_u64_or_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(value_to_u64)
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn value_to_u64(value: Value) -> std::result::Result<u64, String> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| format!("ocekavano nezaporne cele cislo, prislo {number}")),
        Value::String(text) => text
            .parse::<u64>()
            .map_err(|e| format!("ocekavano cislo nebo ciselny retezec, prislo {text:?}: {e}")),
        other => Err(format!(
            "ocekavano cislo nebo ciselny retezec, prislo {other}"
        )),
    }
}

#[derive(Debug, Deserialize)]
struct JiraSearchResponse {
    issues: Option<Vec<JiraIssue>>,
    #[serde(default, rename = "errorMessages")]
    error_messages: Vec<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: String,
    status: JiraStatus,
    updated: String,
}

#[derive(Debug, Deserialize)]
struct JiraStatus {
    name: String,
}

#[derive(Debug)]
struct ChatSummary {
    id: String,
    one_to_one: bool,
    names: BTreeSet<String>,
    topic: Option<String>,
}

struct TeamsClient {
    http: Client,
    cfg: TeamsConfig,
    refresh_token: Option<String>,
    skype_token: Option<String>,
    session_id: String,
    chat_base_url: String,
    chat_msg_url: String,
}

impl TeamsClient {
    fn new(cfg: TeamsConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("jira2teams-rust/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Nelze vytvorit HTTP klienta pro Teams")?;
        let chat_base_url = cfg.chat_afd_url.trim_end_matches('/').to_string();
        let chat_msg_url = cfg.chat_url.trim_end_matches('/').to_string();

        Ok(Self {
            http,
            cfg,
            refresh_token: None,
            skype_token: None,
            session_id: Uuid::new_v4().to_string(),
            chat_base_url,
            chat_msg_url,
        })
    }

    fn interactive_login(&mut self) -> Result<()> {
        let resp = self
            .http
            .post(&self.cfg.device_code_url)
            .form(&[
                ("client_id", self.cfg.oauth_client_id.as_str()),
                ("resource", TEAMS_OAUTH_RESOURCE),
            ])
            .send()
            .context("Teams device-code: request selhal")?;

        let status = resp.status();
        let text = resp
            .text()
            .context("Teams device-code: nelze precist odpoved")?;
        if !status.is_success() {
            bail!(
                "Teams device-code vratil HTTP {}: {}",
                status,
                truncate(&text, 900)
            );
        }

        let device: DeviceCodeResponse =
            serde_json::from_str(&text).context("Teams device-code odpoved neni platny JSON")?;

        println!();
        println!("Microsoft Teams Personal prihlaseni");
        println!("Otevri: {}", device.verification_url);
        println!("Kod:    {}", device.user_code);
        if let Some(message) = device.message.as_deref() {
            println!();
            println!("{}", message);
        }
        println!();
        println!("Cekam na dokonceni prihlaseni...");
        std::io::stdout().flush().ok();

        let deadline = Instant::now() + Duration::from_secs(device.expires_in);
        let mut interval = device.interval.unwrap_or(5).max(1);

        loop {
            if Instant::now() >= deadline {
                bail!("Device code vyprsel. Spust znovu jira2teams --login.");
            }
            thread::sleep(Duration::from_secs(interval));

            let resp = self
                .http
                .post(&self.cfg.v1_token_url)
                .form(&[
                    ("client_id", self.cfg.oauth_client_id.as_str()),
                    ("grant_type", DEVICE_GRANT_TYPE),
                    ("code", device.device_code.as_str()),
                ])
                .send()
                .context("Teams device-code polling selhal")?;

            let status = resp.status();
            let body = resp
                .text()
                .context("Teams device-code: nelze precist token odpoved")?;
            let value: Value = serde_json::from_str(&body).with_context(|| {
                format!(
                    "Teams device-code token odpoved neni JSON: {}",
                    truncate(&body, 700)
                )
            })?;

            if status.is_success() {
                let refresh = value
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| anyhow!("Microsoft prihlaseni nevratilo refresh_token"))?
                    .to_string();

                self.refresh_token = Some(refresh);
                self.save_current_refresh_token()?;
                self.skype_token = None;
                self.refresh_skype_token()
                    .context("Prihlaseni probehlo, ale nelze ziskat Teams Skype token")?;

                println!(
                    "Prihlaseni uspesne. Refresh token ulozen v {}",
                    self.cfg.auth_file.display()
                );
                println!("Dalsi start aplikace uz interaktivni prihlaseni nepotrebuje.");
                return Ok(());
            }

            let error = value.get("error").and_then(Value::as_str).unwrap_or("");
            let description = value
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or("");

            match error {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval = interval.saturating_add(5);
                    continue;
                }
                "authorization_declined" => bail!("Prihlaseni bylo odmitnuto: {description}"),
                "expired_token" | "code_expired" => {
                    bail!("Device code vyprsel. Spust znovu jira2teams --login.")
                }
                _ => bail!(
                    "Teams device-code polling vratil HTTP {}: {}",
                    status,
                    truncate(&body, 900)
                ),
            }
        }
    }

    fn logout(&mut self) -> Result<()> {
        self.refresh_token = None;
        self.skype_token = None;
        match fs::remove_file(&self.cfg.auth_file) {
            Ok(()) => {
                println!(
                    "Teams prihlaseni odstraneno: {}",
                    self.cfg.auth_file.display()
                );
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "Teams auth cache uz neexistuje: {}",
                    self.cfg.auth_file.display()
                );
                Ok(())
            }
            Err(e) => {
                Err(e).with_context(|| format!("Nelze odstranit {}", self.cfg.auth_file.display()))
            }
        }
    }

    fn ensure_login(&mut self) -> Result<()> {
        if self.skype_token.is_some() {
            return Ok(());
        }
        self.load_refresh_token_if_needed()?;
        self.refresh_skype_token()
    }

    fn load_refresh_token_if_needed(&mut self) -> Result<()> {
        if self.refresh_token.is_some() {
            return Ok(());
        }
        let text = fs::read_to_string(&self.cfg.auth_file).with_context(|| {
            format!(
                "Teams neni prihlasen. Chybi {}. Spust jednou: jira2teams --login",
                self.cfg.auth_file.display()
            )
        })?;
        let cache: TeamsAuthCache = serde_json::from_str(&text).with_context(|| {
            format!(
                "Teams auth cache {} neni platny JSON",
                self.cfg.auth_file.display()
            )
        })?;
        if cache.refresh_token.trim().is_empty() {
            bail!("Teams auth cache neobsahuje refresh token. Spust jira2teams --login.");
        }
        self.refresh_token = Some(cache.refresh_token);
        Ok(())
    }

    fn refresh_skype_token(&mut self) -> Result<()> {
        let refresh_token = self
            .refresh_token
            .clone()
            .ok_or_else(|| anyhow!("Chybi Teams refresh token"))?;

        let resp = self
            .http
            .post(&self.cfg.v2_token_url)
            .form(&[
                ("scope", TEAMS_SKYPE_SCOPE),
                ("client_id", self.cfg.oauth_client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
            ])
            .send()
            .context("Teams OAuth refresh request selhal")?;

        let status = resp.status();
        let text = resp
            .text()
            .context("Teams OAuth refresh: nelze precist odpoved")?;
        let value: Value = serde_json::from_str(&text).with_context(|| {
            format!(
                "Teams OAuth refresh odpoved neni JSON: {}",
                truncate(&text, 700)
            )
        })?;

        if !status.is_success() {
            let error = value.get("error").and_then(Value::as_str).unwrap_or("");
            let description = value
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or("");
            if error == "invalid_grant" || error == "interaction_required" {
                bail!(
                    "Teams prihlaseni je nutne obnovit interaktivne. Spust jira2teams --login. Microsoft: {}",
                    truncate(description, 700)
                );
            }
            bail!(
                "Teams OAuth refresh vratil HTTP {}: {}",
                status,
                truncate(&text, 900)
            );
        }

        let access_token = value
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| anyhow!("Teams OAuth refresh neobsahuje access_token"))?
            .to_string();

        if let Some(new_refresh) = value
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
        {
            self.refresh_token = Some(new_refresh.to_string());
            self.save_current_refresh_token()?;
        }

        let resp = self
            .http
            .post(&self.cfg.consumer_auth_url)
            .bearer_auth(&access_token)
            .header("Accept", "application/json; ver=1.0")
            .json(&json!({}))
            .send()
            .context("Teams consumer auth request selhal")?;

        let status = resp.status();
        let text = resp
            .text()
            .context("Teams consumer auth: nelze precist odpoved")?;
        if !status.is_success() {
            bail!(
                "Teams consumer auth selhal na {}: HTTP {}: {}",
                self.cfg.consumer_auth_url,
                status,
                truncate(&text, 900)
            );
        }
        let value: Value = serde_json::from_str(&text).with_context(|| {
            format!(
                "Teams consumer auth odpoved neni JSON: {}",
                truncate(&text, 900)
            )
        })?;
        let token = extract_skype_token(&value).ok_or_else(|| {
            anyhow!(
                "Teams consumer auth odpoved neobsahuje Skype token: {}",
                truncate(&text, 900)
            )
        })?;

        if let Some(region_gtms) = value.get("regionGtms") {
            if let Some(url) = region_gtms
                .get("chatServiceAfd")
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
            {
                self.chat_base_url = url.trim_end_matches('/').to_string();
            }
            if let Some(url) = region_gtms
                .get("chatService")
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
            {
                self.chat_msg_url = url.trim_end_matches('/').to_string();
            }
        }

        self.skype_token = Some(token);
        Ok(())
    }

    fn save_current_refresh_token(&self) -> Result<()> {
        let refresh_token = self
            .refresh_token
            .as_deref()
            .ok_or_else(|| anyhow!("Chybi refresh token pro ulozeni"))?;
        let cache = TeamsAuthCache {
            version: 1,
            refresh_token: refresh_token.to_string(),
            saved_unix: unix_seconds(),
        };
        secure_write_json(&self.cfg.auth_file, &cache)
    }

    fn auth_header(&self) -> Result<String> {
        let token = self
            .skype_token
            .as_deref()
            .ok_or_else(|| anyhow!("Teams token neni inicializovan"))?;
        Ok(format!("skypetoken={token}"))
    }

    fn ensure_ready(&mut self) -> Result<()> {
        // Jira-watch pouze odesila zpravy a cte seznam existujicich chatu.
        // Endpoint registration slouzi hlavne pro Teams/Trouter push notifikace,
        // ktere tento daemon nepotrebuje. Soucasny consumer backend navic muze
        // registration route vracet jako 410 Gone.
        self.ensure_login()?;
        Ok(())
    }

    fn chat_bases(&self) -> Vec<String> {
        let mut bases = Vec::new();
        for base in [&self.chat_base_url, &self.chat_msg_url] {
            let normalized = base.trim_end_matches('/').to_string();
            if !normalized.is_empty() && !bases.iter().any(|item| item == &normalized) {
                bases.push(normalized);
            }
        }
        bases
    }

    fn get_chat_response(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<(StatusCode, String, String)> {
        let auth = self.auth_header()?;
        let bases = self.chat_bases();
        let mut failures = Vec::new();

        for base in bases {
            let url = format!("{base}{path}");
            let request = self
                .http
                .get(&url)
                .header("Authentication", &auth)
                .header("Accept", "application/json")
                .header("BehaviorOverride", "redirectAs404")
                .header("x-ms-session-id", &self.session_id)
                .query(query);
            match request.send() {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().unwrap_or_default();
                    if status.is_success() {
                        return Ok((status, text, base));
                    }
                    failures.push(format!("{url}: HTTP {status}: {}", truncate(&text, 400)));
                }
                Err(e) => failures.push(format!("{url}: {e}")),
            }
        }

        bail!(
            "Teams chat API selhalo na vsech dostupnych endpointech: {}",
            failures.join(" | ")
        )
    }

    fn list_chats(&mut self) -> Result<Vec<ChatSummary>> {
        self.ensure_ready()?;
        let (_status, text, used_base) = self.get_chat_response(
            "/v1/users/ME/conversations",
            &[
                ("startTime", "0"),
                ("pageSize", "100"),
                ("view", "msnp24Equivalent"),
                (
                    "targetType",
                    "Thread|Passport|Skype|Lync|NotificationStream",
                ),
            ],
        )?;

        let value: Value =
            serde_json::from_str(&text).context("Teams seznam chatu neni platny JSON")?;
        let conversations = value
            .get("conversations")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Teams odpoved neobsahuje pole conversations"))?;

        if conversations.is_empty() {
            eprintln!("Teams: endpoint {used_base} vratil prazdny seznam conversations.");
        }

        let mut result = Vec::new();
        for conv in conversations {
            let Some(id) = conv.get("id").and_then(Value::as_str) else {
                continue;
            };
            if id.starts_with("48:") {
                continue;
            }

            let product_type = conv
                .get("productThreadType")
                .and_then(Value::as_str)
                .unwrap_or("");
            let thread_type = conv
                .pointer("/threadProperties/threadType")
                .and_then(Value::as_str)
                .unwrap_or("");
            let one_to_one = conv
                .get("isOneOnOne")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || product_type.eq_ignore_ascii_case("OneToOneChat")
                || thread_type.eq_ignore_ascii_case("chat");

            let topic = conv
                .pointer("/threadProperties/topic")
                .or_else(|| conv.get("topic"))
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .map(ToOwned::to_owned);

            let mut names = BTreeSet::new();
            if let Some(name) = last_message_name(conv) {
                names.insert(name);
            }

            result.push(ChatSummary {
                id: id.to_string(),
                one_to_one,
                names,
                topic,
            });
        }

        Ok(result)
    }

    fn enrich_chat_names(&mut self, chat: &mut ChatSummary) {
        if !chat.one_to_one {
            return;
        }
        let Ok(names) = self.message_display_names(&chat.id, 20) else {
            return;
        };
        chat.names.extend(names);
    }

    fn message_display_names(
        &mut self,
        conversation_id: &str,
        page_size: u32,
    ) -> Result<BTreeSet<String>> {
        self.ensure_ready()?;
        let path = format!(
            "/v1/users/ME/conversations/{}/messages",
            encode(conversation_id)
        );
        let page_size_string = page_size.to_string();
        let (_status, text, _used_base) = self.get_chat_response(
            &path,
            &[
                ("pageSize", page_size_string.as_str()),
                ("view", "superchat"),
            ],
        )?;

        let value: Value =
            serde_json::from_str(&text).context("Teams zpravy chatu nejsou platny JSON")?;
        let mut names = BTreeSet::new();
        if let Some(messages) = value.get("messages").and_then(Value::as_array) {
            for msg in messages {
                if let Some(name) = msg
                    .get("imdisplayname")
                    .or_else(|| msg.get("imDisplayName"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    names.insert(name.to_string());
                }
            }
        }
        Ok(names)
    }

    fn resolve_thread_id(&mut self) -> Result<String> {
        if let Some(id) = &self.cfg.thread_id {
            return Ok(id.clone());
        }

        let target = self
            .cfg
            .target_name
            .clone()
            .ok_or_else(|| anyhow!("Chybi TEAMS_THREAD_ID i TEAMS_TO"))?;
        let target_lower = target.to_lowercase();
        let mut matches = Vec::new();

        for mut chat in self.list_chats()? {
            if !chat.one_to_one {
                continue;
            }
            self.enrich_chat_names(&mut chat);
            if chat
                .names
                .iter()
                .any(|name| name.to_lowercase() == target_lower)
            {
                matches.push(chat.id);
            }
        }

        match matches.len() {
            1 => Ok(matches.remove(0)),
            0 => bail!(
                "Nenalezen 1:1 Teams chat pro TEAMS_TO={target:?}. Pouzij --list-chats a nastav stabilni TEAMS_THREAD_ID."
            ),
            _ => bail!(
                "TEAMS_TO={target:?} odpovida vice 1:1 chatum. Pouzij --list-chats a nastav TEAMS_THREAD_ID."
            ),
        }
    }

    fn send_html(&mut self, html: &str) -> Result<()> {
        let thread_id = self.resolve_thread_id()?;
        self.ensure_ready()?;

        let first = self.send_html_once(&thread_id, html)?;
        if first == StatusCode::UNAUTHORIZED || first == StatusCode::FORBIDDEN {
            self.skype_token = None;
            self.ensure_ready()?;
            let retry = self.send_html_once(&thread_id, html)?;
            if retry.is_success() {
                return Ok(());
            }
            bail!("Teams odeslani po obnoveni tokenu selhalo: HTTP {retry}");
        }

        if !first.is_success() {
            bail!("Teams odeslani selhalo: HTTP {first}");
        }
        Ok(())
    }

    fn send_html_once(&self, thread_id: &str, html: &str) -> Result<StatusCode> {
        let auth = self.auth_header()?;
        let client_message_id = format!(
            "{}{}",
            unix_millis() * 10,
            Uuid::new_v4().as_u128() % 1_000_000
        );
        let payload = json!({
            "content": html,
            "messagetype": "RichText/Html",
            "contenttype": "text",
            "clientmessageid": client_message_id,
            "imdisplayname": "",
            "properties": {
                "importance": "",
                "subject": ""
            }
        });

        let mut last_status = StatusCode::BAD_GATEWAY;
        let mut failures = Vec::new();
        for base in self.chat_bases() {
            let url = format!(
                "{}/v1/users/ME/conversations/{}/messages",
                base,
                encode(thread_id)
            );
            let request = self
                .http
                .post(&url)
                .header("Authentication", &auth)
                .header("Accept", "application/json")
                .header("BehaviorOverride", "redirectAs404")
                .header("x-ms-session-id", &self.session_id)
                .json(&payload);

            match request.send() {
                Ok(resp) => {
                    let status = resp.status();
                    last_status = status;
                    if status.is_success() {
                        return Ok(status);
                    }
                    let text = resp.text().unwrap_or_default();
                    failures.push(format!("{url}: HTTP {status}: {}", truncate(&text, 500)));
                }
                Err(e) => failures.push(format!("{url}: {e}")),
            }
        }

        eprintln!(
            "Teams API odeslani selhalo na vsech endpointech: {}",
            failures.join(" | ")
        );
        Ok(last_status)
    }
}

fn parse_mode() -> Result<Mode> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => Ok(Mode::Watch),
        [arg] if arg == "--once" => Ok(Mode::Once),
        [arg] if arg == "--login" => Ok(Mode::Login),
        [arg] if arg == "--logout" => Ok(Mode::Logout),
        [arg] if arg == "--list-chats" => Ok(Mode::ListChats),
        [arg] if arg == "--test-teams" => Ok(Mode::TestTeams),
        [arg] if arg == "--help" || arg == "-h" => Ok(Mode::Help),
        [arg] if arg == "--version" || arg == "-V" => Ok(Mode::Version),
        _ => bail!("Neznamy parametr. Pouzij --help."),
    }
}

fn main() -> Result<()> {
    let mode = parse_mode()?;
    match mode {
        Mode::Help => {
            print_help();
            return Ok(());
        }
        Mode::Version => {
            println!("jira2teams {VERSION}");
            return Ok(());
        }
        Mode::Login => {
            let cfg = TeamsConfig::from_env(false)?;
            let mut teams = TeamsClient::new(cfg)?;
            teams.interactive_login()?;
            return Ok(());
        }
        Mode::Logout => {
            let cfg = TeamsConfig::from_env(false)?;
            let mut teams = TeamsClient::new(cfg)?;
            teams.logout()?;
            return Ok(());
        }
        Mode::ListChats => {
            let cfg = TeamsConfig::from_env(false)?;
            let mut teams = TeamsClient::new(cfg)?;
            let mut chats = teams.list_chats()?;
            chats.sort_by(|a, b| {
                b.one_to_one
                    .cmp(&a.one_to_one)
                    .then_with(|| a.id.cmp(&b.id))
            });
            if chats.is_empty() {
                println!("Teams nevratil zadne chaty.");
            }
            for chat in &mut chats {
                teams.enrich_chat_names(chat);
                let kind = if chat.one_to_one { "1:1" } else { "chat" };
                let names = if chat.names.is_empty() {
                    "-".to_string()
                } else {
                    chat.names.iter().cloned().collect::<Vec<_>>().join(", ")
                };
                let topic = chat.topic.as_deref().unwrap_or("-");
                println!("{kind}\t{names}\t{topic}\t{}", chat.id);
            }
            return Ok(());
        }
        Mode::TestTeams => {
            let cfg = TeamsConfig::from_env(true)?;
            let mut teams = TeamsClient::new(cfg)?;
            teams.send_html("<b>jira2teams</b><br>Testovaci Teams Personal zprava.")?;
            println!("Testovaci Teams zprava byla odeslana.");
            return Ok(());
        }
        Mode::Watch | Mode::Once => {}
    }

    let cfg = AppConfig::from_env()?;
    let teams_cfg = TeamsConfig::from_env(true)?;
    ensure_parent_dir(&cfg.state_file)?;
    let mut first_run = !cfg.state_file.exists();
    let jira_http = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("jira2teams-rust/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Nelze vytvorit HTTP klienta pro Jira")?;
    let mut teams = TeamsClient::new(teams_cfg)?;

    teams.ensure_ready().context("Teams inicializace selhala")?;

    if mode == Mode::Once {
        check_once(&cfg, &jira_http, &mut teams, &mut first_run)?;
        return Ok(());
    }

    println!(
        "Jira2Teams spusten (kontrola kazdych {} s).",
        cfg.poll_interval
    );
    loop {
        if let Err(e) = check_once(&cfg, &jira_http, &mut teams, &mut first_run) {
            eprintln!(
                "Kontrola selhala: {e:#}. Zkusim to znovu za {} s.",
                cfg.poll_interval
            );
        }
        thread::sleep(Duration::from_secs(cfg.poll_interval));
    }
}

fn check_once(
    cfg: &AppConfig,
    jira_http: &Client,
    teams: &mut TeamsClient,
    first_run: &mut bool,
) -> Result<()> {
    let issues = fetch_jira_issues(cfg, jira_http)?;
    let old_state = load_state(&cfg.state_file)?;

    if !*first_run {
        for issue in &issues {
            let changed = old_state
                .get(&issue.key)
                .map(|last| last != &issue.fields.updated)
                .unwrap_or(true);
            if changed {
                let html = format_jira_message(cfg, issue);
                teams
                    .send_html(&html)
                    .with_context(|| format!("Nelze odeslat Teams notifikaci pro {}", issue.key))?;
                println!("Teams: {} - {}", issue.key, issue.fields.summary);
            }
        }
    }

    let new_state: HashMap<String, String> = issues
        .iter()
        .map(|issue| (issue.key.clone(), issue.fields.updated.clone()))
        .collect();
    save_state(&cfg.state_file, &new_state)?;
    *first_run = false;
    Ok(())
}

fn fetch_jira_issues(cfg: &AppConfig, http: &Client) -> Result<Vec<JiraIssue>> {
    let url = format!("{}/rest/api/3/search/jql", cfg.jira_url);
    let max_results = cfg.max_results.to_string();
    let resp = http
        .get(&url)
        .basic_auth(&cfg.jira_email, Some(&cfg.jira_api_token))
        .header("Accept", "application/json")
        .query(&[
            ("jql", cfg.jql.as_str()),
            ("maxResults", max_results.as_str()),
            ("fields", "summary,status,updated"),
        ])
        .send()
        .context("Jira API request selhal")?;

    let status = resp.status();
    let text = resp.text().context("Nelze precist Jira API odpoved")?;
    if !status.is_success() {
        bail!("Jira API vratilo HTTP {}: {}", status, truncate(&text, 900));
    }

    let parsed: JiraSearchResponse =
        serde_json::from_str(&text).context("Jira API nevratilo ocekavany JSON")?;
    if let Some(issues) = parsed.issues {
        return Ok(issues);
    }

    let detail = if !parsed.error_messages.is_empty() {
        parsed.error_messages.join("; ")
    } else if let Some(message) = parsed.message {
        message
    } else {
        truncate(&text, 900)
    };
    bail!("Chyba Jira API: {detail}")
}

fn format_jira_message(_cfg: &AppConfig, issue: &JiraIssue) -> String {
    let key = html_escape(&issue.key);
    let status = html_escape(&issue.fields.status.name);
    let summary_without_urls = strip_urls(&issue.fields.summary);
    let summary = html_escape(&summary_without_urls);
    format!("<b>{key}: {status}</b><br>{summary}")
}

fn strip_urls(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if lower.contains("http://") || lower.contains("https://") || lower.starts_with("www.")
            {
                "[odkaz odstranen]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn load_state(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("Nelze cist state file {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("State file {} neni platny JSON", path.display()))
}

fn save_state(path: &Path, state: &HashMap<String, String>) -> Result<()> {
    ensure_parent_dir(path)?;
    let data = serde_json::to_vec_pretty(state).context("Nelze serializovat state")?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, data).with_context(|| format!("Nelze zapsat {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "Nelze atomicky prejmenovat {} na {}",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn secure_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent_dir(path)?;
    let data = serde_json::to_vec_pretty(value).context("Nelze serializovat auth cache")?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("Nelze vytvorit {}", tmp.display()))?;
    file.write_all(&data)
        .with_context(|| format!("Nelze zapsat {}", tmp.display()))?;
    file.sync_all()
        .with_context(|| format!("Nelze fsync {}", tmp.display()))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Nelze nastavit prava 0600 na {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "Nelze atomicky prejmenovat {} na {}",
            tmp.display(),
            path.display()
        )
    })?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Nelze nastavit prava 0600 na {}", path.display()))?;
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Nelze vytvorit adresar {}", parent.display()))?;
        }
    }
    Ok(())
}

fn extract_skype_token(value: &Value) -> Option<String> {
    const POINTERS: &[&str] = &[
        "/skypetoken",
        "/skypeToken",
        "/tokens/skypeToken",
        "/tokens/skypetoken",
        "/skypeToken/skypetoken",
        "/skypeToken/skypeToken",
    ];
    for pointer in POINTERS {
        if let Some(token) = value.pointer(pointer).and_then(Value::as_str) {
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn last_message_name(conv: &Value) -> Option<String> {
    conv.pointer("/lastMessage/imdisplayname")
        .or_else(|| conv.pointer("/lastMessage/imDisplayName"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn truncate(input: &str, max_chars: usize) -> String {
    let mut s: String = input.chars().take(max_chars).collect();
    if input.chars().count() > max_chars {
        s.push_str("...");
    }
    s.replace(['\r', '\n'], " ")
}

fn required_env(name: &str) -> Result<String> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => bail!("Nastav promennou {name}"),
    }
}

fn optional_nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn home_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(required_env("HOME")?))
}

fn xdg_config_dir() -> Result<PathBuf> {
    if let Some(path) = optional_nonempty_env("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(path))
    } else {
        Ok(home_dir()?.join(".config"))
    }
}

fn parse_env<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .parse::<T>()
            .map_err(|e| anyhow!("Neplatna hodnota {name}={value:?}: {e}")),
        _ => Ok(default),
    }
}

fn print_help() {
    println!(
        r#"jira2teams {VERSION}

Pouziti:
  jira2teams                 hlida Jira ve smycce; vhodne pod systemd --user
  jira2teams --once          jedna kontrola a konec
  jira2teams --login         jednorazove interaktivni Teams Personal prihlaseni
  jira2teams --logout        smaze lokalni Teams refresh token
  jira2teams --list-chats    vypise Teams chaty a jejich thread ID
  jira2teams --test-teams    posle testovaci 1:1 Teams zpravu
  jira2teams --version

Jira:
  JIRA_URL            napr. https://firma.atlassian.net
  JIRA_EMAIL          Jira ucet
  JIRA_API_TOKEN      Jira API token
  JQL                 volitelne; default = prirazene mne, nevyresene
  POLL_INTERVAL       volitelne; default 60
  MAX_RESULTS         volitelne; default 50
  STATE_FILE          volitelne; default ~/.cache/jira2teams/state.json

Teams Personal (private/unsupported messaging API, bez Microsoft Graph):
  TEAMS_THREAD_ID     doporuceno: stabilni ID existujiciho 1:1 chatu
  TEAMS_TO            alternativa: presne zobrazovane jmeno kontaktu
  TEAMS_AUTH_FILE     volitelne; default ~/.config/jira2teams/teams-auth.json

Prihlaseni:
  Heslo se do aplikace nezadava ani neuklada.
  Jednou spust jira2teams --login a dokonci Microsoft device-code login v prohlizeci.
  Refresh token se ulozi s pravy 0600 a prezije restart aplikace i pocitace.
  Aplikace ho pri pouziti automaticky rotuje/obnovuje.

Volitelne override endpointu:
  TEAMS_OAUTH_CLIENT_ID
  TEAMS_DEVICE_CODE_URL
  TEAMS_V1_TOKEN_URL
  TEAMS_V2_TOKEN_URL
  TEAMS_AUTH_URL
  TEAMS_CHAT_URL

Doporuceny postup:
  1. jira2teams --login
  2. jira2teams --list-chats
  3. nastav TEAMS_THREAD_ID
  4. jira2teams --test-teams
  5. spust jako systemd --user sluzbu
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_code_accepts_numeric_timing_fields() {
        let parsed: DeviceCodeResponse = serde_json::from_str(
            r#"{
                "device_code":"device",
                "user_code":"ABCD-EFGH",
                "verification_url":"https://microsoft.com/devicelogin",
                "expires_in":900,
                "interval":5,
                "message":null
            }"#,
        )
        .expect("numeric device-code response must parse");

        assert_eq!(parsed.expires_in, 900);
        assert_eq!(parsed.interval, Some(5));
        assert_eq!(parsed.user_code, "ABCD-EFGH");
    }

    #[test]
    fn device_code_accepts_string_timing_fields_and_verification_uri_alias() {
        let parsed: DeviceCodeResponse = serde_json::from_str(
            r#"{
                "device_code":"device",
                "user_code":"ABCD-EFGH",
                "verification_uri":"https://microsoft.com/devicelogin",
                "expires_in":"900",
                "interval":"5",
                "message":"login"
            }"#,
        )
        .expect("string device-code response must parse");

        assert_eq!(parsed.expires_in, 900);
        assert_eq!(parsed.interval, Some(5));
        assert_eq!(parsed.verification_url, "https://microsoft.com/devicelogin");
    }

    #[test]
    fn strip_urls_removes_urls_from_teams_notification_text() {
        assert_eq!(
            strip_urls("pred https://example.invalid/a po www.example.invalid konec"),
            "pred [odkaz odstranen] po [odkaz odstranen] konec"
        );
        assert_eq!(strip_urls("bez odkazu"), "bez odkazu");
    }

    #[test]
    fn html_escape_escapes_message_markup() {
        assert_eq!(
            html_escape(r#"<tag a="b">&'x'"#),
            "&lt;tag a=&quot;b&quot;&gt;&amp;&#39;x&#39;"
        );
    }

    #[test]
    fn extract_skype_token_supports_nested_consumer_response() {
        let value = json!({
            "skypeToken": {
                "skypetoken": "token-value"
            }
        });
        assert_eq!(extract_skype_token(&value).as_deref(), Some("token-value"));
    }

    #[test]
    fn truncate_is_character_safe_and_flattens_lines() {
        assert_eq!(truncate("abcdef", 3), "abc...");
        assert_eq!(truncate("a\nb\r", 10), "a b ");
        assert_eq!(truncate("ěščřž", 3), "ěšč...");
    }
}
