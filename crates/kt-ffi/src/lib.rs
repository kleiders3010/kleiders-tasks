//! Kotlin bridge (UniFFI) for Kleiders Tasks.
//!
//! One [`KtNode`] per device: the app layer (groups, lists, tasks, invites) plus every
//! transport (Nostr with the self-sustaining relay list, P2P, DHT, helpers), running on an
//! internal tokio runtime. Kotlin calls are blocking and short; network work happens in
//! [`KtNode::sync_now`] (background jobs) or in live mode ([`KtNode::start_live`], app open),
//! which calls [`KtListener::on_change`] whenever something arrived.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kt_engine::invite::CodeKind;
use kt_engine::replica::Event;
use kt_engine::sandbox::SandboxConfig;
use kt_engine::{DocId, GroupId};
use kt_net::dht::DhtDiscovery;
use kt_net::helper::HelperMailbox;
use kt_net::helper_dir::HelperDirectory;
use kt_net::invite_net::InviteLayers;
use kt_net::mailbox::Mailbox;
use kt_net::nostr::{NostrMailbox, NostrPolicy};
use kt_net::p2p::{P2p, P2pConfig};
use kt_net::relays::RelayDirectory;
use kt_net::settings::{NetSettings, Platform as NetPlatform};
use kt_net::sync::Syncer;
use kt_tasks::app::{App, ListInfo};

uniffi::setup_scaffolding!();

#[cfg(target_os = "android")]
mod android;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum KtError {
    #[error("{msg}")]
    Failed { msg: String },
}

fn fail<E: std::fmt::Display>(e: E) -> KtError {
    KtError::Failed { msg: e.to_string() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Platform {
    Desktop,
    Phone,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct NodeOptions {
    pub platform: Platform,
    /// Nostr seed relays (empty = the shipped seed list).
    pub relays: Vec<String>,
    /// Direct device-to-device sync (iroh). Off in some tests.
    pub p2p: bool,
    /// Mainline DHT discovery.
    pub dht: bool,
    /// Bind address for P2P ("" = all interfaces; tests use "127.0.0.1:0").
    pub bind: String,
    /// This device's name for the account's other devices ("Pixel 7", the host name).
    pub device_name: String,
    /// Where the daily sealed backup goes ("" = none). Android: a folder that Android
    /// backup includes (and nothing else). A backup found here on a fresh install is
    /// restored automatically.
    pub backup_dir: String,
}

/// What a link code joins, shown before the user confirms (D6).
#[derive(Clone, Debug, uniffi::Record)]
pub struct LinkPreview {
    /// The account's display name.
    pub account_name: String,
    /// Its devices and lists: "Pixel 7 · Mark's PC · 12 lists (Work, Groceries, …)".
    pub summary: String,
}

/// A device of this account.
#[derive(Clone, Debug, uniffi::Record)]
pub struct DeviceItem {
    pub id: String,
    pub name: String,
    pub this_device: bool,
}

/// A device linked recently ("New device linked · Undo").
#[derive(Clone, Debug, uniffi::Record)]
pub struct RecentLinkItem {
    pub device_id: String,
    pub name: String,
    pub linked_ms: u64,
}

/// Something another person did (for notifications; batched per list by the platform).
#[derive(Clone, Debug, uniffi::Record)]
pub struct ChangeItem {
    pub group_name: String,
    /// Empty for joins.
    pub list_id: String,
    pub list_name: String,
    pub who: String,
    /// "added", "completed" or "joined".
    pub kind: String,
    pub title: String,
    pub at_ms: u64,
}

/// The settings screen (DESIGN settings table). A setting that doesn't exist on this
/// platform is `None` and isn't shown at all.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Settings {
    /// Off = sync only while the app is open.
    pub sync_in_background: bool,
    /// "Notify me when others change a list".
    pub notify_changes: bool,
    /// "Help others sync": one of `help_options`.
    pub help_others: String,
    /// "wifi_and_charging", "wifi_while_open" (phones), "on" (desktops), "off".
    pub help_options: Vec<String>,
    /// "Relay connections for others" (D51): desktops only.
    pub relay_for_others: Option<bool>,
    /// "Start with computer": desktops only.
    pub start_with_computer: Option<bool>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct StoredSettings {
    sync_in_background: bool,
    notify_changes: bool,
    net: NetSettings,
    start_with_computer: bool,
}

impl StoredSettings {
    fn defaults(p: NetPlatform) -> StoredSettings {
        StoredSettings { sync_in_background: true, notify_changes: true, net: NetSettings::defaults(p), start_with_computer: true }
    }
}

fn help_name(h: kt_net::settings::HelpOthers) -> &'static str {
    use kt_net::settings::HelpOthers::*;
    match h {
        WifiAndCharging => "wifi_and_charging",
        WifiWhileOpen => "wifi_while_open",
        On => "on",
        Off => "off",
    }
}

/// A newer version is out (from the static update manifest).
#[derive(Clone, Debug, uniffi::Record)]
pub struct UpdateItem {
    pub version_name: String,
    /// Our own store listing or release page, nothing else.
    pub link: String,
}

/// What a restore did.
#[derive(Clone, Debug, uniffi::Record)]
pub struct RestoreInfo {
    /// Shared groups: readable now, back in when a member's device sees the request.
    pub rejoining_groups: u32,
    /// Solo lists copied into this device's own lists.
    pub rehomed_lists: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct GroupItem {
    pub id: String,
    pub name: String,
    pub personal: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ListItem {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub group_name: String,
}

/// A task in display order, with its depth (children follow their parent).
#[derive(Clone, Debug, uniffi::Record)]
pub struct TaskItem {
    pub id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub title: String,
    pub done: bool,
    pub notes: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct InviteInfo {
    pub words: Vec<String>,
    pub expires_ms: u64,
    /// The same code as a link (and QR content), with this device's addresses when known.
    pub link: String,
}

/// A QR code as a square of modules (the UI draws it).
#[derive(Clone, Debug, uniffi::Record)]
pub struct QrMatrix {
    pub size: u32,
    /// Row by row, `size * size` entries; true = dark.
    pub dark: Vec<bool>,
}

/// Android: is the app Context registered for iroh's DNS resolver and network monitor?
/// (Always true elsewhere.)
#[uniffi::export]
pub fn android_context_ready() -> bool {
    #[cfg(target_os = "android")]
    return android::context_ready();
    #[cfg(not(target_os = "android"))]
    true
}

/// Encode text (a share link) as a QR code.
#[uniffi::export]
pub fn qr_matrix(text: String) -> Result<QrMatrix, KtError> {
    let q = qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::M).map_err(fail)?;
    let size = q.width() as u32;
    let dark = q.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect();
    Ok(QrMatrix { size, dark })
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct JoinResult {
    pub group_name: String,
    pub inviter_name: String,
    /// True if already in; false if waiting for a member's device to let us in.
    pub joined: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncReport {
    pub written: u32,
    pub received: u32,
    pub rejected: u32,
    pub peers_reached: u32,
    pub joined_groups: Vec<String>,
}

/// One relay's health as this device sees it (hidden diagnostics screen).
#[derive(Clone, Debug, uniffi::Record)]
pub struct RelayStat {
    pub url: String,
    pub published_ok: u64,
    pub connect_failures: u64,
    pub failures_in_a_row: u64,
    pub readback_hits: u64,
    pub readback_misses: u64,
}

/// Something changed that the UI should show (lists, tasks, membership).
#[uniffi::export(with_foreign)]
pub trait KtListener: Send + Sync {
    fn on_change(&self);
}

struct Net {
    syncer: Arc<Syncer>,
    nostr: Arc<NostrMailbox>,
    relays: RelayDirectory,
    p2p: Option<Arc<P2p>>,
    helpers: Option<(Arc<HelperMailbox>, HelperDirectory)>,
    dht: Option<Arc<DhtDiscovery>>,
}

#[derive(uniffi::Object)]
pub struct KtNode {
    app: App,
    rt: tokio::runtime::Runtime,
    net: Net,
    data_dir: PathBuf,
    listener: Mutex<Option<Arc<dyn KtListener>>>,
    /// The link record last previewed (confirmed with the same words).
    previewed: Mutex<Option<(String, kt_engine::invite::InviteRecord)>>,
    /// Live mode: the stop signal and the task holding a reference to this node.
    live: Mutex<Option<(tokio::sync::watch::Sender<bool>, tokio::task::JoinHandle<()>)>>,
    backup_dir: Option<PathBuf>,
    restored: Option<RestoreInfo>,
    settings: Mutex<StoredSettings>,
    /// (on Wi-Fi, charging, app open), as the platform last reported (phones).
    device_state: Mutex<(bool, bool, bool)>,
    /// Desktops: the relay run for others (D51), when the setting is on.
    #[cfg(feature = "desktop")]
    relay_for_others: tokio::sync::Mutex<Option<kt_net::relay_runner::RelayForOthers>>,
    /// Real networking (not a loopback test node): only then does a desktop relay for others.
    #[cfg(feature = "desktop")]
    public_net: bool,
}

const BACKUP_FILE: &str = "tasks.ktbackup";
const BACKUP_EVERY: Duration = Duration::from_secs(24 * 3600);
const RECOVERY_DAYS: u32 = 30;

fn gid(s: &str) -> Result<GroupId, KtError> {
    GroupId::from_hex(s).ok_or_else(|| fail("bad group id"))
}

fn flatten(ts: &[kt_tasks::TaskView], parent: Option<&str>, depth: u32, out: &mut Vec<TaskItem>) {
    for t in ts {
        out.push(TaskItem { id: t.id.clone(), parent_id: parent.map(str::to_string), depth, title: t.title.clone(), done: t.done, notes: t.notes.clone() });
        flatten(&t.children, Some(&t.id), depth + 1, out);
    }
}

impl KtNode {
    fn list(&self, list_id: &str) -> Result<ListInfo, KtError> {
        let d = DocId::from_hex(list_id).ok_or_else(|| fail("bad list id"))?;
        self.app.lists().into_iter().find(|l| l.doc == d).ok_or_else(|| fail("no such list"))
    }

    /// (name, our display name) per group for invite records. The personal group is
    /// advertised by its summary: that's what a link code's confirmation screen shows.
    fn names(&self) -> std::collections::HashMap<GroupId, (String, String)> {
        let me = self.app.display_name();
        let personal = self.app.personal_group();
        let summary = self.app.account_summary();
        self.app.groups().into_iter().map(|g| (g.group, (if g.group == personal { summary.clone() } else { g.name }, me.clone()))).collect()
    }

    fn layers(&self) -> InviteLayers {
        InviteLayers {
            p2p: self.net.p2p.clone(),
            dht: self.net.dht.clone(),
            nostr: Some(self.net.nostr.clone()),
            helpers: self.net.helpers.as_ref().map(|h| h.0.clone()),
            lan: None,
        }
    }

    /// The daily sealed backup (the previous one is kept next to it).
    fn backup_if_due(&self) {
        let Some(dir) = &self.backup_dir else { return };
        let path = dir.join(BACKUP_FILE);
        let age = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        if age.is_some_and(|a| a < BACKUP_EVERY) {
            return;
        }
        let _ = self.write_backup_to(&path);
    }

    fn write_backup_to(&self, path: &std::path::Path) -> Result<(), KtError> {
        let bytes = self.app.backup(RECOVERY_DAYS).map_err(fail)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(fail)?;
        }
        if path.exists() {
            let _ = std::fs::copy(path, path.with_extension("prev.ktbackup"));
        }
        kt_engine::storage::write_atomic(path, &bytes).map_err(fail)
    }

    fn invite_info(&self, code: &kt_engine::invite::Code, kind: CodeKind, expires_ms: u64) -> InviteInfo {
        let here = self.net.p2p.as_ref().map(|p| p.where_data());
        InviteInfo {
            words: code.to_words().into_iter().map(str::to_string).collect(),
            expires_ms,
            link: kt_net::invite_net::share_url(code, kind, here.as_ref()),
        }
    }

    /// Helping follows the setting and the device's state.
    fn apply_helping(&self) {
        let (wifi, charging, open) = *self.device_state.lock().expect("lock");
        let on = self.settings.lock().expect("lock").net.helper_limits(wifi, charging, open).is_some();
        if let Some(p) = &self.net.p2p {
            p.set_helping(on);
        }
    }

    /// "Relay connections for others" (desktop, D51): run the relay while the setting is
    /// on. It's announced only if strangers can reach it (relay_runner); checking that
    /// takes a few seconds, so it happens in the background.
    fn apply_relay_for_others(self: &Arc<Self>) {
        #[cfg(feature = "desktop")]
        {
            let want = self.public_net && self.settings.lock().expect("lock").net.relay_config().is_some();
            let Some(p2p) = self.net.p2p.clone() else { return };
            let me = self.clone();
            self.rt.spawn(async move {
                let mut cur = me.relay_for_others.lock().await;
                match (want, cur.is_some()) {
                    (true, false) => {
                        let any: std::net::IpAddr = std::net::Ipv4Addr::UNSPECIFIED.into();
                        *cur = kt_net::relay_runner::RelayForOthers::start(&p2p, any, None).await.ok();
                    }
                    (false, true) => {
                        if let Some(r) = cur.take() {
                            r.stop(&p2p).await;
                        }
                    }
                    _ => {}
                }
            });
        }
    }

    fn save_settings(&self, s: &StoredSettings) -> Result<(), KtError> {
        kt_engine::storage::write_atomic(&self.data_dir.join("settings"), &kt_engine::wire::encode(*b"SE", 1, s).map_err(fail)?).map_err(fail)
    }

    fn notify(&self) {
        if let Some(l) = self.listener.lock().expect("lock").clone() {
            l.on_change();
        }
    }

    /// One full round on every layer: maintenance, sync, invites.
    async fn round(&self) -> SyncReport {
        let net = &self.net;
        // First, so a new recovery invite goes out with this same round.
        self.backup_if_due();
        kt_net::relays::maintain(&net.syncer, &net.nostr, &net.relays).await;
        if let Some((h, dir)) = &net.helpers {
            kt_net::helper_dir::maintain_with_dht(&net.syncer, h, &net.nostr, dir, net.dht.as_deref()).await;
        }
        if let (Some(d), Some(p)) = (&net.dht, &net.p2p) {
            p.set_dht_entry(d.entry_point().await); // members bootstrap through each other
        }
        if let Some(d) = &net.dht {
            d.publish().await;
            d.lookup().await;
            d.save_cache().await;
        }
        let (w, r) = net.syncer.sync_once().await;
        let mut peers = 0;
        if let Some(p) = &net.p2p {
            p.publish_whereabouts();
            let pr = p.sync_round().await;
            peers = pr.peers_reached as u32;
        }
        let names = self.names();
        let names_fn = |g: &GroupId| names.get(g).cloned().unwrap_or_default();
        let layers = self.layers();
        if kt_net::invite_net::publish(&net.syncer, &layers, &names_fn).await.records > 0 {
            kt_net::invite_net::publish_ahead(&net.syncer, &layers, &names_fn).await;
        }
        let joined = self.app.with(|r| r.settle_joins()).into_iter().map(|p| p.group_name).collect();
        let _ = self.app.settle_account();
        let _ = self.app.publish_names();
        kt_net::health::exchange(&net.syncer, net.helpers.as_ref().map(|h| h.0.as_ref()), &net.nostr);
        SyncReport { written: w.written as u32, received: r.slots_opened as u32, rejected: r.rejected as u32, peers_reached: peers, joined_groups: joined }
    }
}

#[uniffi::export]
impl KtNode {
    /// Open this device. `identity_dir` must be excluded from backups; `worker_path` is the
    /// import sandbox executable (Android: `nativeLibraryDir/libkt_import.so`).
    #[uniffi::constructor]
    pub fn open(identity_dir: String, data_dir: String, worker_path: String, options: NodeOptions) -> Result<Arc<KtNode>, KtError> {
        // A fresh install with a backup waiting (Android restored it): restore it.
        let found = (!options.backup_dir.is_empty() && !PathBuf::from(&data_dir).join("app.meta").exists())
            .then(|| std::fs::read(PathBuf::from(&options.backup_dir).join(BACKUP_FILE)).ok())
            .flatten();
        KtNode::start(identity_dir, data_dir, worker_path, options, found, false)
    }

    /// Restore a backup file the user picked (first launch, nothing made on this device yet).
    #[uniffi::constructor]
    pub fn open_restoring(identity_dir: String, data_dir: String, worker_path: String, options: NodeOptions, backup: Vec<u8>) -> Result<Arc<KtNode>, KtError> {
        kt_engine::replica::backup::open(&backup).map_err(|_| fail("not a Tasks backup file"))?;
        KtNode::start(identity_dir, data_dir, worker_path, options, Some(backup), true)
    }
}

impl KtNode {
    fn start(identity_dir: String, data_dir: String, worker_path: String, options: NodeOptions, restore: Option<Vec<u8>>, picked: bool) -> Result<Arc<KtNode>, KtError> {
        let sandbox = SandboxConfig { worker: PathBuf::from(worker_path), timeout: Duration::from_secs(20) };
        let data = PathBuf::from(&data_dir);
        let identity = PathBuf::from(identity_dir);
        let (app, restored) = match restore.map(|b| App::restore(&identity, &data, sandbox.clone(), &b)) {
            Some(Ok((app, s))) => (app, Some(RestoreInfo { rejoining_groups: s.rejoining.len() as u32, rehomed_lists: s.rehomed_lists as u32 })),
            // A file the user picked: say what went wrong.
            Some(Err(e)) if picked => return Err(fail(e)),
            Some(Err(_)) => {
                // An unreadable backup never keeps the app from starting, and isn't
                // overwritten by the next daily backup.
                if let Some(dir) = (!options.backup_dir.is_empty()).then(|| PathBuf::from(&options.backup_dir)) {
                    let _ = std::fs::rename(dir.join(BACKUP_FILE), dir.join("tasks.unrestored.ktbackup"));
                }
                if !data.join("app.meta").exists() {
                    let _ = std::fs::remove_dir_all(&data);
                    std::fs::create_dir_all(&data).map_err(fail)?;
                }
                (App::open(&identity, &data, sandbox).map_err(fail)?, None)
            }
            None => (App::open(&identity, &data, sandbox).map_err(fail)?, None),
        };
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().worker_threads(2).build().map_err(fail)?;
        let seeds: Vec<String> = if options.relays.is_empty() { kt_net::relays::SEED_RELAYS.iter().map(|s| s.to_string()).collect() } else { options.relays.clone() };
        let replica = app.replica();
        let platform = match options.platform {
            Platform::Desktop => NetPlatform::Desktop,
            Platform::Phone => NetPlatform::Phone,
        };
        let stored = std::fs::read(data.join("settings"))
            .ok()
            .and_then(|b| kt_engine::wire::decode::<StoredSettings>(*b"SE", 1, &b).ok())
            .map(|x| x.1)
            .filter(|s| s.net.platform() == platform)
            .unwrap_or_else(|| StoredSettings::defaults(platform));
        let settings = stored.net.clone();
        let net = rt.block_on(async {
            let refs: Vec<&str> = seeds.iter().map(|s| s.as_str()).collect();
            let nostr = Arc::new(NostrMailbox::new(&refs, NostrPolicy::default()));
            let syncer = Syncer::build(
                replica.clone(),
                vec![nostr.clone() as Arc<dyn Mailbox>],
                Some(data.join("sync.state")),
                Some(data.join("slots.store")),
            );
            let relays = RelayDirectory::new(&refs, Some(data.join("relays.dir")));
            if seeds.iter().all(|u| u.starts_with("wss://")) {
                kt_net::relays::install_resolver(&syncer, &nostr);
            }
            let mut p2p = None;
            let mut helpers = None;
            if options.p2p {
                let key = kt_engine::crypto::derive(&replica.lock().expect("lock").identity().secret().to_bytes(), "kt/helper-disk-key/v1", b"");
                // The store exists whenever helping is on at all; whether it serves right now
                // follows the device's state (set_device_state).
                let helper = settings.helper_limits_with_disk(true, true, true, &data.join("helping"), key);
                let cfg = P2pConfig {
                    bind: (!options.bind.is_empty()).then(|| options.bind.parse().ok()).flatten(),
                    portmap: options.bind.is_empty(),
                    helper,
                    // Best-effort public layers (D50), off for loopback tests.
                    n0_relays: options.bind.is_empty(),
                    n0_dns: options.bind.is_empty(),
                    ..Default::default()
                };
                if let Ok(p) = P2p::start(syncer.clone(), cfg).await {
                    p.publish_whereabouts();
                    let hmb = Arc::new(HelperMailbox::new(p.clone(), vec![]));
                    syncer.add_mailbox(hmb.clone() as Arc<dyn Mailbox>);
                    kt_net::helper_dir::install_resolver(&syncer, &hmb);
                    helpers = Some((hmb, HelperDirectory::new(Some(data.join("helpers.dir")))));
                    p2p = Some(p);
                }
            }
            let dht = if options.dht { DhtDiscovery::with_cache(replica.clone(), data.join("dht_nodes.txt"), None).ok().map(Arc::new) } else { None };
            Net { syncer, nostr, relays, p2p, helpers, dht }
        });
        if !options.device_name.is_empty() {
            let _ = app.set_device_name(&options.device_name);
        }
        let backup_dir = (!options.backup_dir.is_empty()).then(|| PathBuf::from(&options.backup_dir));
        let node = Arc::new(KtNode {
            app,
            rt,
            net,
            data_dir: data,
            listener: Mutex::new(None),
            previewed: Mutex::new(None),
            live: Mutex::new(None),
            backup_dir,
            restored,
            settings: Mutex::new(stored),
            // Phones start as "not on Wi-Fi" until the platform says otherwise.
            device_state: Mutex::new((platform == NetPlatform::Desktop, false, false)),
            #[cfg(feature = "desktop")]
            relay_for_others: tokio::sync::Mutex::new(None),
            #[cfg(feature = "desktop")]
            public_net: options.bind.is_empty(),
        });
        node.apply_helping();
        node.apply_relay_for_others();
        Ok(node)
    }
}

#[uniffi::export]
impl KtNode {
    /// Set when this node was opened from a backup.
    pub fn restored(&self) -> Option<RestoreInfo> {
        self.restored.clone()
    }

    /// Nothing made on this device yet: the first-launch screen may offer a restore.
    pub fn is_pristine(&self) -> bool {
        self.app.is_pristine()
    }

    /// The list as Markdown (Obsidian Tasks format).
    pub fn export_markdown(&self, list_id: String) -> Result<String, KtError> {
        let l = self.list(&list_id)?;
        self.app.export_markdown(&l).map_err(fail)
    }

    /// The list as a sealed `.tasklist` file (with history; re-importable).
    pub fn export_list_file(&self, list_id: String) -> Result<Vec<u8>, KtError> {
        let l = self.list(&list_id)?;
        self.app.export_list_file(&l).map_err(fail)
    }

    /// Import a `.tasklist` file as a new list (in `group_id`, or among your own lists).
    pub fn import_list_file(&self, group_id: Option<String>, file: Vec<u8>) -> Result<ListItem, KtError> {
        let g = group_id.as_deref().map(gid).transpose()?;
        let l = self.app.import_list_file(g, &file).map_err(fail)?;
        self.notify();
        Ok(ListItem { id: l.doc.to_hex(), group_id: l.group.to_hex(), name: l.name, group_name: l.group_name })
    }

    /// Write a sealed backup now (export, or before a risky operation).
    pub fn write_backup(&self, path: String) -> Result<(), KtError> {
        self.write_backup_to(&PathBuf::from(path))
    }

    pub fn device_key(&self) -> String {
        let p = self.app.with(|r| r.device_public());
        format!("{}{}", hex::encode(p.sign), hex::encode(p.kex))
    }

    pub fn display_name(&self) -> String {
        self.app.display_name()
    }

    pub fn set_display_name(&self, name: String) -> Result<(), KtError> {
        self.app.set_display_name(&name).map_err(fail)
    }

    pub fn groups(&self) -> Vec<GroupItem> {
        self.app.groups().into_iter().map(|g| GroupItem { id: g.group.to_hex(), name: g.name, personal: g.personal }).collect()
    }

    pub fn create_group(&self, name: String) -> Result<GroupItem, KtError> {
        let g = self.app.create_group(&name).map_err(fail)?;
        Ok(GroupItem { id: g.group.to_hex(), name: g.name, personal: false })
    }

    pub fn lists(&self) -> Vec<ListItem> {
        self.app.lists().into_iter().map(|l| ListItem { id: l.doc.to_hex(), group_id: l.group.to_hex(), name: l.name, group_name: l.group_name }).collect()
    }

    /// Create a list in a group (`None` = a solo list).
    pub fn create_list(&self, group_id: Option<String>, name: String) -> Result<ListItem, KtError> {
        let g = group_id.map(|s| gid(&s)).transpose()?;
        let l = self.app.create_list(g, &name).map_err(fail)?;
        Ok(ListItem { id: l.doc.to_hex(), group_id: l.group.to_hex(), name: l.name, group_name: l.group_name })
    }

    pub fn tasks(&self, list_id: String) -> Result<Vec<TaskItem>, KtError> {
        let l = self.list(&list_id)?;
        let ts = self.app.read(&l, kt_tasks::tasks).map_err(fail)?;
        let mut out = Vec::new();
        flatten(&ts, None, 0, &mut out);
        Ok(out)
    }

    pub fn add_task(&self, list_id: String, title: String, parent_id: Option<String>) -> Result<String, KtError> {
        let l = self.list(&list_id)?;
        self.app.edit(&l, |d| kt_tasks::add_task(d, parent_id.as_deref(), &title)).map_err(fail)
    }

    pub fn set_done(&self, list_id: String, task_id: String, done: bool) -> Result<(), KtError> {
        let l = self.list(&list_id)?;
        self.app.edit(&l, |d| kt_tasks::set_done(d, &task_id, done)).map_err(fail)
    }

    pub fn rename_task(&self, list_id: String, task_id: String, title: String) -> Result<(), KtError> {
        let l = self.list(&list_id)?;
        self.app.edit(&l, |d| kt_tasks::set_title(d, &task_id, &title)).map_err(fail)
    }

    pub fn delete_task(&self, list_id: String, task_id: String) -> Result<(), KtError> {
        let l = self.list(&list_id)?;
        self.app.edit(&l, |d| kt_tasks::delete_task(d, &task_id)).map_err(fail)
    }

    /// Create an invite to a group (8 words, valid `days`, 1–30).
    pub fn invite(&self, group_id: String, days: u32) -> Result<InviteInfo, KtError> {
        let g = gid(&group_id)?;
        if g == self.app.personal_group() {
            return Err(fail("your personal group is for your own devices (device linking)"));
        }
        let ms = u64::from(days.clamp(1, 30)) * 86_400_000;
        let (code, expires) = self.app.with(|r| r.create_invite(&g, CodeKind::Invite, ms)).map_err(fail)?;
        Ok(self.invite_info(&code, CodeKind::Invite, expires))
    }

    /// Autocomplete one word of a code (3+ letters): the full word, if unique.
    pub fn complete_word(&self, prefix: String) -> Option<String> {
        kt_engine::invite::complete(&prefix).map(|(_, w)| w.to_string())
    }

    /// Join a group with an invite's 8 words (network: seconds).
    /// Join with an invite's words or its link (typed, pasted or scanned).
    pub fn join_code(&self, words: String) -> Result<JoinResult, KtError> {
        let shared = kt_net::invite_net::parse_shared(&words).map_err(fail)?;
        if shared.kind == Some(CodeKind::Link) {
            return Err(fail("this is a device link, not an invite: use it to link this device to your account"));
        }
        let code = shared.code;
        let direct: Vec<_> = shared.direct.into_iter().collect();
        self.rt.block_on(async {
            let layers = self.layers();
            let found = kt_net::invite_net::fetch(&layers, &code, CodeKind::Invite, &direct, Duration::from_millis(0)).await;
            let Some((rec, _)) = found else {
                if kt_net::invite_net::fetch(&layers, &code, CodeKind::Link, &[], Duration::from_millis(0)).await.is_some() {
                    return Err(fail("this is a device link code, not an invite: use it to link this device to your account"));
                }
                return Err(fail("no invite found for these words: check them, or it may have expired"));
            };
            let joined = kt_net::invite_net::join(&self.net.syncer, self.net.p2p.as_ref(), &rec).await.map_err(fail)?;
            self.net.syncer.sync_once().await;
            Ok(JoinResult { group_name: rec.group_name, inviter_name: rec.inviter_name, joined })
        })
    }

    /// Others' changes since the last call (background sync posts them as notifications).
    /// Changes older than a few days are dropped; none with "Notify me" off.
    pub fn take_changes(&self) -> Vec<ChangeItem> {
        let changes = self.app.take_changes().unwrap_or_default();
        if !self.settings.lock().expect("lock").notify_changes {
            return Vec::new();
        }
        changes
            .into_iter()
            .map(|c| ChangeItem {
                group_name: c.group_name,
                list_id: c.list.as_ref().map(|l| l.doc.to_hex()).unwrap_or_default(),
                list_name: c.list.map(|l| l.name).unwrap_or_default(),
                who: c.who,
                kind: match c.kind {
                    kt_tasks::app::ChangeKind::Added => "added",
                    kt_tasks::app::ChangeKind::Completed => "completed",
                    kt_tasks::app::ChangeKind::Joined => "joined",
                }
                .into(),
                title: c.title,
                at_ms: c.at_ms,
            })
            .collect()
    }

    /// The settings screen's current values.
    pub fn settings(&self) -> Settings {
        let s = self.settings.lock().expect("lock").clone();
        let desktop = s.net.platform() == NetPlatform::Desktop;
        Settings {
            sync_in_background: s.sync_in_background,
            notify_changes: s.notify_changes,
            help_others: help_name(s.net.help_others).into(),
            help_options: s.net.help_options().into_iter().map(|h| help_name(h).into()).collect(),
            relay_for_others: s.net.relay_setting_available().then(|| s.net.relay_for_others()),
            start_with_computer: desktop.then_some(s.start_with_computer),
        }
    }

    /// Save the settings screen. Settings that don't exist on this platform are refused.
    pub fn set_settings(self: Arc<Self>, new: Settings) -> Result<(), KtError> {
        let mut s = self.settings.lock().expect("lock").clone();
        s.sync_in_background = new.sync_in_background;
        s.notify_changes = new.notify_changes;
        let h = s.net.help_options().into_iter().find(|h| help_name(*h) == new.help_others).ok_or_else(|| fail("not a choice on this device"))?;
        s.net.set_help_others(h).map_err(fail)?;
        if let Some(r) = new.relay_for_others {
            s.net.set_relay_for_others(r).map_err(fail)?;
        }
        match (new.start_with_computer, s.net.platform()) {
            (Some(v), NetPlatform::Desktop) => s.start_with_computer = v,
            (Some(_), _) => return Err(fail("desktop only")),
            (None, _) => {}
        }
        if h == kt_net::settings::HelpOthers::Off {
            kt_net::helper::wipe_disk(&self.data_dir.join("helping")); // disk doesn't clear itself
        }
        self.save_settings(&s)?;
        *self.settings.lock().expect("lock") = s;
        self.apply_helping();
        self.apply_relay_for_others();
        Ok(())
    }

    /// Phones report Wi-Fi and charging (and the node knows when the app is open): helping
    /// others follows them (§7a).
    pub fn set_device_state(&self, on_wifi: bool, charging: bool) {
        self.device_state.lock().expect("lock").0 = on_wifi;
        self.device_state.lock().expect("lock").1 = charging;
        self.apply_helping();
    }

    /// Is there a newer version than `current_version_code`? `channel`: "play", "apk",
    /// "windows" or "linux" (which link the notice opens). Reads the static manifest
    /// (apps.jurisgames.com, then GitHub).
    pub fn check_update(&self, current_version_code: u64, channel: String) -> Option<UpdateItem> {
        use kt_net::update::{check, Channel, MANIFEST_URLS};
        let ch = match channel.as_str() {
            "play" => Channel::Play,
            "apk" => Channel::Apk,
            "windows" => Channel::Windows,
            "linux" => Channel::Linux,
            _ => return None,
        };
        self.rt
            .block_on(check(&MANIFEST_URLS, "tasks", current_version_code, ch))
            .map(|u| UpdateItem { version_name: u.version_name, link: u.link })
    }

    /// The user is looking: nothing so far needs a notification.
    pub fn mark_seen(&self) {
        let _ = self.app.mark_seen();
    }

    /// A link code for adding another device to this account (valid a few hours).
    pub fn link_code(&self) -> Result<InviteInfo, KtError> {
        let (code, expires_ms) = self.app.create_link_code().map_err(fail)?;
        Ok(self.invite_info(&code, CodeKind::Link, expires_ms))
    }

    /// Find the account a link code joins (the confirmation screen shows it).
    pub fn link_preview(&self, words: String) -> Result<LinkPreview, KtError> {
        let shared = kt_net::invite_net::parse_shared(&words).map_err(fail)?;
        if shared.kind == Some(CodeKind::Invite) {
            return Err(fail("this is an invite, not a device link: use \"Join with code\""));
        }
        let code = shared.code;
        let direct: Vec<_> = shared.direct.into_iter().collect();
        let rec = self.rt.block_on(async {
            let layers = self.layers();
            match kt_net::invite_net::fetch(&layers, &code, CodeKind::Link, &direct, Duration::from_millis(0)).await {
                Some((rec, _)) => Ok(rec),
                None if kt_net::invite_net::fetch(&layers, &code, CodeKind::Invite, &[], Duration::from_millis(0)).await.is_some() => {
                    Err(fail("this is an invite, not a device link code: use \"Join with code\""))
                }
                None => Err(fail("no link code found for these words: check them, or it may have expired")),
            }
        })?;
        let preview = LinkPreview { account_name: rec.inviter_name.clone(), summary: rec.group_name.clone() };
        *self.previewed.lock().expect("lock") = Some((code.to_text(), rec));
        Ok(preview)
    }

    /// The user confirmed the previewed account: link this device to it. Its lists merge
    /// into the account once a device of the account lets it in.
    pub fn link_confirm(&self, words: String) -> Result<(), KtError> {
        let code = kt_net::invite_net::parse_shared(&words).map_err(fail)?.code.to_text();
        let rec = match self.previewed.lock().expect("lock").take() {
            Some((c, rec)) if c == code => rec,
            _ => return Err(fail("preview the code first")),
        };
        self.app.begin_link(&rec).map_err(fail)?;
        self.rt.block_on(async {
            kt_net::invite_net::join_direct(self.net.p2p.as_ref(), &rec).await;
            self.net.syncer.sync_once().await;
        });
        self.notify();
        Ok(())
    }

    /// Linking into another account and not in yet.
    pub fn is_linking(&self) -> bool {
        self.app.linking().is_some()
    }

    /// This account's devices.
    pub fn devices(&self) -> Vec<DeviceItem> {
        self.app.devices().into_iter().map(|d| DeviceItem { id: d.device.to_hex(), name: d.name, this_device: d.this_device }).collect()
    }

    /// Devices linked in the last day ("New device linked · Undo" on every device).
    pub fn recent_links(&self) -> Vec<RecentLinkItem> {
        self.app.recent_links().into_iter().map(|l| RecentLinkItem { device_id: l.device.to_hex(), name: l.name, linked_ms: l.linked_ms }).collect()
    }

    /// Undo a link: the device leaves the account and every group (keys rotate).
    pub fn unlink_device(&self, device_id: String) -> Result<(), KtError> {
        let d = kt_engine::DeviceId::from_hex(&device_id).ok_or_else(|| fail("bad device id"))?;
        self.app.unlink_device(d).map_err(fail)?;
        self.notify();
        Ok(())
    }

    /// One sync round on every layer (background jobs call this).
    pub fn sync_now(&self) -> SyncReport {
        let rep = self.rt.block_on(self.round());
        self.notify();
        rep
    }

    pub fn set_listener(&self, listener: Arc<dyn KtListener>) {
        *self.listener.lock().expect("lock") = Some(listener);
    }

    /// App open: live updates on every layer until [`KtNode::stop_live`].
    pub fn start_live(self: Arc<Self>) {
        self.device_state.lock().expect("lock").2 = true;
        self.apply_helping();
        let (tx, rx) = tokio::sync::watch::channel(false);
        self.stop_live();
        let net_syncer = self.net.syncer.clone();
        let nostr = self.net.nostr.clone();
        let p2p = self.net.p2p.clone();
        self.rt.spawn(kt_net::live::run_live(net_syncer.clone(), nostr, p2p.clone(), rx.clone()));
        if let Some(p) = p2p {
            self.rt.spawn(kt_net::live::run_p2p_live(net_syncer, p, rx.clone()));
        }
        // Periodic full rounds + change notifications.
        let me = self.clone();
        let mut stop = rx;
        let task = self.rt.spawn(async move {
            let mut last_round = std::time::Instant::now() - Duration::from_secs(3600);
            loop {
                if last_round.elapsed() > Duration::from_secs(45) {
                    me.round().await;
                    last_round = std::time::Instant::now();
                }
                let changed = me.app.with(|r| r.take_events()).iter().any(|e| {
                    matches!(e, Event::Imported { .. } | Event::DeviceJoined { .. } | Event::DocEpochSwitched { .. } | Event::EpochAccepted { .. })
                });
                if changed {
                    let _ = me.app.mark_seen(); // the app is open: shown, not notified
                    me.notify();
                }
                tokio::select! {
                    _ = stop.changed() => return,
                    _ = tokio::time::sleep(Duration::from_millis(400)) => {}
                }
            }
        });
        *self.live.lock().expect("lock") = Some((tx, task));
    }

    /// Back to background mode. Returns once live mode has let go of this node (so the
    /// platform can close it, e.g. to restore a backup).
    pub fn stop_live(&self) {
        self.device_state.lock().expect("lock").2 = false;
        self.apply_helping();
        if let Some((tx, task)) = self.live.lock().expect("lock").take() {
            let _ = tx.send(true);
            task.abort();
            let _ = self.rt.block_on(async { tokio::time::timeout(Duration::from_secs(5), task).await });
        }
    }

    /// Relay health (diagnostics).
    pub fn relay_stats(&self) -> Vec<RelayStat> {
        let mut v: Vec<RelayStat> = self
            .net
            .nostr
            .health()
            .into_iter()
            .map(|(url, h)| RelayStat {
                url,
                published_ok: h.published_ok,
                connect_failures: h.connect_failures,
                failures_in_a_row: h.failures_in_a_row,
                readback_hits: h.readback_hits,
                readback_misses: h.readback_misses,
            })
            .collect();
        v.sort_by(|a, b| a.url.cmp(&b.url));
        v
    }

    pub fn data_dir(&self) -> String {
        self.data_dir.display().to_string()
    }
}
