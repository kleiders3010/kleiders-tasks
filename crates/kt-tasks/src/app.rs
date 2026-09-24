//! App layer: a person's lists on one device.
//!
//! Model: **every list is its own group** (the sharing unit; a solo list is a group with one
//! member). The **personal group** (DESIGN §5) contains only this person's devices and holds
//! a `profile` document (display name) and an `index` document (the person's list groups),
//! so linked devices (M3) know which groups to join.

use std::path::{Path, PathBuf};

use kt_engine::crypto::DevicePublic;
use kt_engine::replica::Replica;
use kt_engine::sandbox::SandboxConfig;
use kt_engine::{storage, wire, DocId, GroupId};
use loro::{LoroValue, ValueOrContainer};
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

/// A list as the UI sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListInfo {
    pub group: GroupId,
    pub doc: DocId,
    pub name: String,
}

fn eng<E: std::fmt::Display>(e: E) -> Error {
    Error::Invalid(e.to_string())
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
                replica.doc(&personal, &profile).unwrap().get_map("profile").insert("name", "").map_err(eng)?;
                replica.doc(&personal, &index).unwrap().get_map("groups").insert("_", true).map_err(eng)?;
                replica.commit(&personal, &profile).map_err(eng)?;
                replica.commit(&personal, &index).map_err(eng)?;
                let m = AppMeta { personal, profile, index };
                storage::write_atomic(&meta_path, &wire::encode(*b"AM", 1, &m).map_err(eng)?).map_err(eng)?;
                m
            }
        };
        Ok(App { replica, meta })
    }

    pub fn personal_group(&self) -> GroupId {
        self.meta.personal
    }

    pub fn display_name(&self) -> String {
        let doc = self.replica.doc(&self.meta.personal, &self.meta.profile);
        match doc.and_then(|d| d.get_map("profile").get("name")) {
            Some(ValueOrContainer::Value(LoroValue::String(s))) => s.to_string(),
            _ => String::new(),
        }
    }

    pub fn set_display_name(&mut self, name: &str) -> Result<()> {
        let (g, d) = (self.meta.personal, self.meta.profile);
        self.replica.doc(&g, &d).ok_or(Error::NotFound)?.get_map("profile").insert("name", name).map_err(eng)?;
        self.replica.commit(&g, &d).map_err(eng)
    }

    /// Create a new (solo) list: its own group with one list document.
    pub fn create_list(&mut self, name: &str) -> Result<ListInfo> {
        let g = self.replica.create_group().map_err(eng)?;
        let d = self.replica.create_doc(&g).map_err(eng)?;
        crate::init_list(self.replica.doc(&g, &d).unwrap(), name)?;
        self.replica.commit(&g, &d).map_err(eng)?;
        self.register_group(g)?;
        Ok(ListInfo { group: g, doc: d, name: name.to_string() })
    }

    fn register_group(&mut self, g: GroupId) -> Result<()> {
        let (pg, idx) = (self.meta.personal, self.meta.index);
        self.replica.doc(&pg, &idx).ok_or(Error::NotFound)?.get_map("groups").insert(&g.to_hex(), true).map_err(eng)?;
        self.replica.commit(&pg, &idx).map_err(eng)
    }

    /// All lists on this device (every non-personal group's list document).
    pub fn lists(&self) -> Vec<ListInfo> {
        let mut out = Vec::new();
        for g in self.replica.groups() {
            if g == self.meta.personal {
                continue;
            }
            for d in self.replica.docs(&g) {
                if let Some(doc) = self.replica.doc(&g, &d) {
                    out.push(ListInfo { group: g, doc: d, name: crate::list_name(doc) });
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name).then(a.group.cmp(&b.group)));
        out
    }

    /// Find a list by (a prefix of) its group ID or by exact name.
    pub fn find_list(&self, key: &str) -> Result<ListInfo> {
        let lists = self.lists();
        let by_id: Vec<&ListInfo> = lists.iter().filter(|l| l.group.to_hex().starts_with(key)).collect();
        if by_id.len() == 1 {
            return Ok(by_id[0].clone());
        }
        let by_name: Vec<&ListInfo> = lists.iter().filter(|l| l.name == key).collect();
        if by_name.len() == 1 {
            return Ok(by_name[0].clone());
        }
        Err(Error::Invalid(format!("no single list matches '{key}'")))
    }

    /// Run an edit on a list's document and commit it.
    pub fn edit<T>(&mut self, list: &ListInfo, f: impl FnOnce(&loro::LoroDoc) -> Result<T>) -> Result<T> {
        let doc = self.replica.doc(&list.group, &list.doc).ok_or(Error::NotFound)?;
        let out = f(doc)?;
        self.replica.commit(&list.group, &list.doc).map_err(eng)?;
        Ok(out)
    }

    /// Manual sharing for the M1 demo: add another device to a list's group and return the
    /// messages it needs to join. (Invites and device linking replace this in M3.)
    pub fn share_with_device(&mut self, list: &ListInfo, device: DevicePublic) -> Result<Vec<Vec<u8>>> {
        self.replica.add_member(&list.group, device).map_err(eng)?;
        self.replica.export_group(&list.group).map_err(eng)
    }

    /// Accept a join bundle produced by [`App::share_with_device`].
    pub fn join_from_bundle(&mut self, group: GroupId, messages: &[Vec<u8>]) -> Result<()> {
        self.replica.accept_group(group);
        for m in messages {
            let _ = self.replica.receive(m);
        }
        self.register_group(group)
    }
}
