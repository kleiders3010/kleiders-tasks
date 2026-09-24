//! App layer: a person's groups and lists on one device.
//!
//! Model (DESIGN §4/§5):
//! - A **group** is a circle of people (e.g. "Family"). Each group has one **group document**
//!   (`kind = group`: name + list index) and **any number of list documents**.
//! - The **personal group** contains only this person's devices. It holds the `profile`
//!   document (display name), the `index` document (the person's groups, so linked devices
//!   know what to join), and the person's **solo lists**.
//! - Moving a list into another group copies it there (lists in different groups are
//!   encrypted with different keys) and removes it from the old group.
//!
//! Every document carries `meta.kind` so the app can tell lists from group/profile/index docs.

use std::path::{Path, PathBuf};

use kt_engine::crypto::DevicePublic;
use kt_engine::replica::Replica;
use kt_engine::sandbox::SandboxConfig;
use kt_engine::{storage, wire, DocId, GroupId};
use loro::{LoroDoc, LoroValue, ValueOrContainer};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, TaskSemantics};

#[derive(Serialize, Deserialize, Clone, Copy)]
struct AppMeta {
    personal: GroupId,
    profile: DocId,
    index: DocId,
}

pub struct App {
    pub replica: Replica,
    meta: AppMeta,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupInfo {
    pub group: GroupId,
    pub name: String,
    pub personal: bool,
}

/// A list as the UI sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListInfo {
    pub group: GroupId,
    pub doc: DocId,
    pub name: String,
    pub group_name: String,
}

fn eng<E: std::fmt::Display>(e: E) -> Error {
    Error::Invalid(e.to_string())
}

fn str_of(v: Option<ValueOrContainer>) -> Option<String> {
    match v? {
        ValueOrContainer::Value(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn doc_kind(doc: &LoroDoc) -> Option<String> {
    str_of(doc.get_map("meta").get("kind"))
}

impl App {
    /// Open the app. `identity_dir` must be excluded from backups (D1).
    pub fn open(identity_dir: &Path, data_dir: &Path, sandbox: SandboxConfig) -> Result<App> {
        let mut replica = Replica::open(identity_dir, data_dir, sandbox).map_err(eng)?;
        replica.set_semantics(Box::new(TaskSemantics));
        let meta_path: PathBuf = data_dir.join("app.meta");
        let meta = match std::fs::read(&meta_path) {
            Ok(b) => wire::decode::<AppMeta>(*b"AM", 1, &b).map_err(eng)?.1,
            Err(_) => {
                let personal = replica.create_group().map_err(eng)?;
                let profile = replica.create_doc(&personal).map_err(eng)?;
                let index = replica.create_doc(&personal).map_err(eng)?;
                {
                    let p = replica.doc(&personal, &profile).unwrap();
                    p.get_map("meta").insert("kind", "profile").map_err(eng)?;
                    p.get_map("profile").insert("name", "").map_err(eng)?;
                    let i = replica.doc(&personal, &index).unwrap();
                    i.get_map("meta").insert("kind", "index").map_err(eng)?;
                    i.get_map("groups").insert(&personal.to_hex(), true).map_err(eng)?;
                }
                replica.commit(&personal, &profile).map_err(eng)?;
                replica.commit(&personal, &index).map_err(eng)?;
                let mut app = App { replica, meta: AppMeta { personal, profile, index } };
                app.create_group_doc(personal, "Personal")?;
                storage::write_atomic(&meta_path, &wire::encode(*b"AM", 1, &app.meta).map_err(eng)?).map_err(eng)?;
                return Ok(app);
            }
        };
        Ok(App { replica, meta })
    }

    /// Hand the replica to a transport driver (which needs shared ownership).
    pub fn into_replica(self) -> Replica {
        self.replica
    }

    fn create_group_doc(&mut self, g: GroupId, name: &str) -> Result<DocId> {
        let d = self.replica.create_doc(&g).map_err(eng)?;
        {
            let doc = self.replica.doc(&g, &d).unwrap();
            doc.get_map("meta").insert("kind", "group").map_err(eng)?;
            doc.get_map("meta").insert("name", name).map_err(eng)?;
            doc.get_map("lists").insert("_", true).map_err(eng)?;
        }
        self.replica.commit(&g, &d).map_err(eng)?;
        Ok(d)
    }

    fn group_doc(&self, g: &GroupId) -> Option<DocId> {
        self.replica
            .docs(g)
            .into_iter()
            .filter(|d| self.replica.doc(g, d).and_then(doc_kind).as_deref() == Some("group"))
            .min()
    }

    pub fn personal_group(&self) -> GroupId {
        self.meta.personal
    }

    pub fn display_name(&self) -> String {
        self.replica
            .doc(&self.meta.personal, &self.meta.profile)
            .and_then(|d| str_of(d.get_map("profile").get("name")))
            .unwrap_or_default()
    }

    pub fn set_display_name(&mut self, name: &str) -> Result<()> {
        let (g, d) = (self.meta.personal, self.meta.profile);
        self.replica.doc(&g, &d).ok_or(Error::NotFound)?.get_map("profile").insert("name", name).map_err(eng)?;
        self.replica.commit(&g, &d).map_err(eng)
    }

    /// Create a shared group (a circle of people) with its group document.
    pub fn create_group(&mut self, name: &str) -> Result<GroupInfo> {
        let g = self.replica.create_group().map_err(eng)?;
        self.create_group_doc(g, name)?;
        self.register_group(g)?;
        Ok(GroupInfo { group: g, name: name.to_string(), personal: false })
    }

    fn register_group(&mut self, g: GroupId) -> Result<()> {
        let (pg, idx) = (self.meta.personal, self.meta.index);
        self.replica.doc(&pg, &idx).ok_or(Error::NotFound)?.get_map("groups").insert(&g.to_hex(), true).map_err(eng)?;
        self.replica.commit(&pg, &idx).map_err(eng)
    }

    pub fn groups(&self) -> Vec<GroupInfo> {
        let mut out: Vec<GroupInfo> = self
            .replica
            .groups()
            .into_iter()
            .filter_map(|g| {
                let gd = self.group_doc(&g)?;
                let name = self.replica.doc(&g, &gd).and_then(|d| str_of(d.get_map("meta").get("name"))).unwrap_or_default();
                Some(GroupInfo { group: g, name, personal: g == self.meta.personal })
            })
            .collect();
        out.sort_by(|a, b| b.personal.cmp(&a.personal).then(a.name.cmp(&b.name)));
        out
    }

    /// Create a list in `group`, or a solo list in the personal group.
    pub fn create_list(&mut self, group: Option<GroupId>, name: &str) -> Result<ListInfo> {
        let g = group.unwrap_or(self.meta.personal);
        let gd = self.group_doc(&g).ok_or(Error::NotFound)?;
        let d = self.replica.create_doc(&g).map_err(eng)?;
        {
            let doc = self.replica.doc(&g, &d).unwrap();
            crate::init_list(doc, name)?;
            doc.get_map("meta").insert("kind", "list").map_err(eng)?;
        }
        self.replica.commit(&g, &d).map_err(eng)?;
        self.replica.doc(&g, &gd).unwrap().get_map("lists").insert(&d.to_hex(), true).map_err(eng)?;
        self.replica.commit(&g, &gd).map_err(eng)?;
        let group_name = self.groups().into_iter().find(|x| x.group == g).map(|x| x.name).unwrap_or_default();
        Ok(ListInfo { group: g, doc: d, name: name.to_string(), group_name })
    }

    /// All lists on this device, across all groups.
    pub fn lists(&self) -> Vec<ListInfo> {
        let mut out = Vec::new();
        for gi in self.groups() {
            let Some(gd) = self.group_doc(&gi.group) else { continue };
            let Some(gdoc) = self.replica.doc(&gi.group, &gd) else { continue };
            let listed: Vec<DocId> = match gdoc.get_map("lists").get_deep_value() {
                LoroValue::Map(m) => m
                    .iter()
                    .filter(|(_, v)| matches!(v, LoroValue::Bool(true)))
                    .filter_map(|(k, _)| DocId::from_hex(k))
                    .collect(),
                _ => Vec::new(),
            };
            for d in listed {
                if let Some(doc) = self.replica.doc(&gi.group, &d) {
                    out.push(ListInfo { group: gi.group, doc: d, name: crate::list_name(doc), group_name: gi.name.clone() });
                }
            }
        }
        out.sort_by(|a, b| a.group_name.cmp(&b.group_name).then(a.name.cmp(&b.name)));
        out
    }

    /// Find a list by exact name or by a prefix of its document ID.
    pub fn find_list(&self, key: &str) -> Result<ListInfo> {
        let lists = self.lists();
        for pred in [
            Box::new(|l: &ListInfo| l.name == key) as Box<dyn Fn(&ListInfo) -> bool>,
            Box::new(|l: &ListInfo| l.doc.to_hex().starts_with(key)),
        ] {
            let m: Vec<&ListInfo> = lists.iter().filter(|l| pred(l)).collect();
            if m.len() == 1 {
                return Ok(m[0].clone());
            }
        }
        Err(Error::Invalid(format!("no single list matches '{key}'")))
    }

    /// Find a group by exact name or by a prefix of its ID.
    pub fn find_group(&self, key: &str) -> Result<GroupInfo> {
        let gs = self.groups();
        let m: Vec<&GroupInfo> = gs.iter().filter(|g| g.name == key || g.group.to_hex().starts_with(key)).collect();
        if m.len() == 1 {
            Ok(m[0].clone())
        } else {
            Err(Error::Invalid(format!("no single group matches '{key}'")))
        }
    }

    /// Run an edit on a list's document and commit it.
    pub fn edit<T>(&mut self, list: &ListInfo, f: impl FnOnce(&LoroDoc) -> Result<T>) -> Result<T> {
        let doc = self.replica.doc(&list.group, &list.doc).ok_or(Error::NotFound)?;
        let out = f(doc)?;
        self.replica.commit(&list.group, &list.doc).map_err(eng)?;
        Ok(out)
    }

    /// Move a list into another group: copy its live state (stable task IDs kept) and
    /// remove it from the old group's index.
    pub fn move_list(&mut self, list: &ListInfo, to: GroupId) -> Result<ListInfo> {
        if list.group == to {
            return Ok(list.clone());
        }
        let peer = self.replica.identity().peer_id();
        let copy = crate::rebuild(self.replica.doc(&list.group, &list.doc).ok_or(Error::NotFound)?, peer)?;
        let snapshot = copy.export(loro::ExportMode::Snapshot).map_err(eng)?;
        let gd_to = self.group_doc(&to).ok_or(Error::NotFound)?;
        let d = self.replica.create_doc(&to).map_err(eng)?;
        self.replica.doc(&to, &d).unwrap().import(&snapshot).map_err(eng)?;
        self.replica.commit(&to, &d).map_err(eng)?;
        self.replica.doc(&to, &gd_to).unwrap().get_map("lists").insert(&d.to_hex(), true).map_err(eng)?;
        self.replica.commit(&to, &gd_to).map_err(eng)?;
        let gd_from = self.group_doc(&list.group).ok_or(Error::NotFound)?;
        self.replica.doc(&list.group, &gd_from).unwrap().get_map("lists").insert(&list.doc.to_hex(), false).map_err(eng)?;
        self.replica.commit(&list.group, &gd_from).map_err(eng)?;
        let group_name = self.groups().into_iter().find(|x| x.group == to).map(|x| x.name).unwrap_or_default();
        Ok(ListInfo { group: to, doc: d, name: list.name.clone(), group_name })
    }

    /// Manual sharing for the prototype: add another device to a group and return the
    /// messages it needs to join. (Invites and device linking replace this in M3.)
    pub fn share_group_with_device(&mut self, group: GroupId, device: DevicePublic) -> Result<Vec<Vec<u8>>> {
        if group == self.meta.personal {
            return Err(Error::Invalid("the personal group is only for your own devices (device linking, M3)".into()));
        }
        self.replica.add_member(&group, device).map_err(eng)?;
        self.replica.export_group(&group).map_err(eng)
    }

    /// Accept a join bundle produced by [`App::share_group_with_device`].
    pub fn join_from_bundle(&mut self, group: GroupId, messages: &[Vec<u8>]) -> Result<()> {
        self.replica.accept_group(group);
        for m in messages {
            let _ = self.replica.receive(m);
        }
        self.register_group(group)
    }
}
