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
    live: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

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

    fn names(&self) -> std::collections::HashMap<GroupId, (String, String)> {
        let me = self.app.display_name();
        self.app.groups().into_iter().map(|g| (g.group, (g.name, me.clone()))).collect()
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

    fn notify(&self) {
        if let Some(l) = self.listener.lock().expect("lock").clone() {
            l.on_change();
        }
    }

    /// One full round on every layer: maintenance, sync, invites.
    async fn round(&self) -> SyncReport {
        let net = &self.net;
        kt_net::relays::maintain(&net.syncer, &net.nostr, &net.relays).await;
        if let Some((h, dir)) = &net.helpers {
            kt_net::helper_dir::maintain_with_dht(&net.syncer, h, &net.nostr, dir, net.dht.as_deref()).await;
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
        SyncReport { written: w.written as u32, received: r.slots_opened as u32, rejected: r.rejected as u32, peers_reached: peers, joined_groups: joined }
    }
}

#[uniffi::export]
impl KtNode {
    /// Open this device. `identity_dir` must be excluded from backups; `worker_path` is the
    /// import sandbox executable (Android: `nativeLibraryDir/libkt_import.so`).
    #[uniffi::constructor]
    pub fn open(identity_dir: String, data_dir: String, worker_path: String, options: NodeOptions) -> Result<Arc<KtNode>, KtError> {
        let sandbox = SandboxConfig { worker: PathBuf::from(worker_path), timeout: Duration::from_secs(20) };
        let data = PathBuf::from(&data_dir);
        let app = App::open(&PathBuf::from(identity_dir), &data, sandbox).map_err(fail)?;
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().worker_threads(2).build().map_err(fail)?;
        let seeds: Vec<String> = if options.relays.is_empty() { kt_net::relays::SEED_RELAYS.iter().map(|s| s.to_string()).collect() } else { options.relays.clone() };
        let replica = app.replica();
        let settings = NetSettings::defaults(match options.platform {
            Platform::Desktop => NetPlatform::Desktop,
            Platform::Phone => NetPlatform::Phone,
        });
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
                let helper = settings.helper_limits_with_disk(true, true, true, &data.join("helping"), key);
                let cfg = P2pConfig {
                    bind: (!options.bind.is_empty()).then(|| options.bind.parse().ok()).flatten(),
                    portmap: options.bind.is_empty(),
                    helper,
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
        Ok(Arc::new(KtNode { app, rt, net, data_dir: data, listener: Mutex::new(None), live: Mutex::new(None) }))
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
        Ok(InviteInfo { words: code.to_words().into_iter().map(str::to_string).collect(), expires_ms: expires })
    }

    /// Autocomplete one word of a code (3+ letters): the full word, if unique.
    pub fn complete_word(&self, prefix: String) -> Option<String> {
        kt_engine::invite::complete(&prefix).map(|(_, w)| w.to_string())
    }

    /// Join a group with an invite's 8 words (network: seconds).
    pub fn join_code(&self, words: String) -> Result<JoinResult, KtError> {
        let code = kt_engine::invite::Code::parse(&words).map_err(fail)?;
        self.rt.block_on(async {
            let layers = self.layers();
            let found = kt_net::invite_net::fetch(&layers, &code, CodeKind::Invite, &[], Duration::from_millis(0)).await;
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
        let (tx, rx) = tokio::sync::watch::channel(false);
        if let Some(old) = self.live.lock().expect("lock").replace(tx) {
            let _ = old.send(true);
        }
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
        self.rt.spawn(async move {
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
                    me.notify();
                }
                tokio::select! {
                    _ = stop.changed() => return,
                    _ = tokio::time::sleep(Duration::from_millis(400)) => {}
                }
            }
        });
    }

    pub fn stop_live(&self) {
        if let Some(tx) = self.live.lock().expect("lock").take() {
            let _ = tx.send(true);
        }
    }

    pub fn data_dir(&self) -> String {
        self.data_dir.display().to_string()
    }
}
