//! kt-tasks: the task-list domain on top of kt-engine (thin, app-specific).
//!
//! Schema of one list document:
//! - map `meta`: `name` (string), `hue` (i64 0–360 or absent), `icon` (string or absent)
//! - tree `tasks`: each node's meta map holds
//!   - `id`: stable task ID (random hex); survives document epochs, which recreate tree nodes
//!   - `title` (string), `done` (bool), `notes` (string, inline up to the attachment threshold)
//!
//! Task order is the tree's fractional-index order. Unknown fields are preserved
//! (principle 8): the domain never deletes keys it doesn't know.

use std::collections::BTreeMap;

pub mod app;

use kt_engine::limits::TEXT_ATTACHMENT_THRESHOLD;
use loro::{LoroDoc, LoroMap, LoroTree, LoroValue, TreeID, ValueOrContainer};
use rand::RngCore;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("task not found")]
    NotFound,
    #[error("loro: {0}")]
    Loro(String),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn loro<E: std::fmt::Display>(e: E) -> Error {
    Error::Loro(e.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub notes: String,
    pub children: Vec<TaskView>,
}

fn tree(doc: &LoroDoc) -> LoroTree {
    let t = doc.get_tree("tasks");
    t.enable_fractional_index(0);
    t
}

fn new_task_id() -> String {
    let mut b = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

fn get_str(m: &LoroMap, key: &str) -> Option<String> {
    match m.get(key)? {
        ValueOrContainer::Value(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn get_bool(m: &LoroMap, key: &str) -> Option<bool> {
    match m.get(key)? {
        ValueOrContainer::Value(LoroValue::Bool(b)) => Some(b),
        _ => None,
    }
}

/// Initialize a new list document.
pub fn init_list(doc: &LoroDoc, name: &str) -> Result<()> {
    let meta = doc.get_map("meta");
    meta.insert("name", name).map_err(loro)?;
    tree(doc);
    doc.commit();
    Ok(())
}

pub fn list_name(doc: &LoroDoc) -> String {
    get_str(&doc.get_map("meta"), "name").unwrap_or_default()
}

pub fn rename_list(doc: &LoroDoc, name: &str) -> Result<()> {
    doc.get_map("meta").insert("name", name).map_err(loro)?;
    doc.commit();
    Ok(())
}

fn find(doc: &LoroDoc, task_id: &str) -> Result<TreeID> {
    let t = tree(doc);
    for n in t.nodes() {
        if t.is_node_deleted(&n).unwrap_or(true) {
            continue;
        }
        if let Ok(meta) = t.get_meta(n) {
            if get_str(&meta, "id").as_deref() == Some(task_id) {
                return Ok(n);
            }
        }
    }
    Err(Error::NotFound)
}

/// Add a task at the end of `parent` (or the top level). Returns its stable ID.
pub fn add_task(doc: &LoroDoc, parent: Option<&str>, title: &str) -> Result<String> {
    let t = tree(doc);
    let parent_node = match parent {
        Some(p) => Some(find(doc, p)?),
        None => None,
    };
    let index = t.children_num(parent_node).unwrap_or(0);
    let node = t.create_at(parent_node, index).map_err(loro)?;
    let meta = t.get_meta(node).map_err(loro)?;
    let id = new_task_id();
    meta.insert("id", id.as_str()).map_err(loro)?;
    meta.insert("title", title).map_err(loro)?;
    meta.insert("done", false).map_err(loro)?;
    doc.commit();
    Ok(id)
}

pub fn set_title(doc: &LoroDoc, task: &str, title: &str) -> Result<()> {
    let n = find(doc, task)?;
    tree(doc).get_meta(n).map_err(loro)?.insert("title", title).map_err(loro)?;
    doc.commit();
    Ok(())
}

pub fn set_done(doc: &LoroDoc, task: &str, done: bool) -> Result<()> {
    let n = find(doc, task)?;
    tree(doc).get_meta(n).map_err(loro)?.insert("done", done).map_err(loro)?;
    doc.commit();
    Ok(())
}

/// Set a task's notes. Text above the attachment threshold (D4) must go through the blob
/// layer instead; until that exists (M2) it's refused with a clear message, never silently cut.
pub fn set_notes(doc: &LoroDoc, task: &str, notes: &str) -> Result<()> {
    if notes.len() > TEXT_ATTACHMENT_THRESHOLD {
        return Err(Error::Invalid("notes this long are stored as an attachment (not available yet)".into()));
    }
    let n = find(doc, task)?;
    tree(doc).get_meta(n).map_err(loro)?.insert("notes", notes).map_err(loro)?;
    doc.commit();
    Ok(())
}

pub fn delete_task(doc: &LoroDoc, task: &str) -> Result<()> {
    let n = find(doc, task)?;
    tree(doc).delete(n).map_err(loro)?;
    doc.commit();
    Ok(())
}

/// Move a task under `parent` (or the top level) at position `index`.
pub fn move_task(doc: &LoroDoc, task: &str, parent: Option<&str>, index: usize) -> Result<()> {
    let n = find(doc, task)?;
    let p = match parent {
        Some(p) => Some(find(doc, p)?),
        None => None,
    };
    tree(doc).mov_to(n, p, index).map_err(loro)?;
    doc.commit();
    Ok(())
}

fn view_of(t: &LoroTree, n: TreeID) -> Option<TaskView> {
    let meta = t.get_meta(n).ok()?;
    let children = t.children(Some(n)).unwrap_or_default().into_iter().filter_map(|c| view_of(t, c)).collect();
    Some(TaskView {
        id: get_str(&meta, "id").unwrap_or_default(),
        title: get_str(&meta, "title").unwrap_or_default(),
        done: get_bool(&meta, "done").unwrap_or(false),
        notes: get_str(&meta, "notes").unwrap_or_default(),
        children,
    })
}

/// All tasks in display order, nested.
pub fn tasks(doc: &LoroDoc) -> Vec<TaskView> {
    let t = tree(doc);
    t.roots().into_iter().filter_map(|n| view_of(&t, n)).collect()
}

/// Canonical, ID-stable representation of a list's live state. Two documents with equal
/// normalized states show the same list, even if their Loro internals differ.
pub fn normalized(doc: &LoroDoc) -> Vec<u8> {
    fn walk(t: &LoroTree, n: TreeID, depth: usize, out: &mut String) {
        if let Ok(meta) = t.get_meta(n) {
            let v = meta.get_deep_value();
            out.push_str(&format!("{depth}|{v:?}\n"));
        }
        for c in t.children(Some(n)).unwrap_or_default() {
            walk(t, c, depth + 1, out);
        }
    }
    let t = tree(doc);
    let mut out = format!("meta|{:?}\n", doc.get_map("meta").get_deep_value());
    for r in t.roots() {
        walk(&t, r, 0, &mut out);
    }
    out.into_bytes()
}

/// Build a fresh document with the same live state (document epoch, D24): no tombstones,
/// no history. Stable task IDs, order and unknown fields are preserved.
pub fn rebuild(doc: &LoroDoc, peer: u64) -> Result<LoroDoc> {
    let fresh = LoroDoc::new();
    fresh.set_peer_id(peer).map_err(loro)?;
    let src_meta = doc.get_map("meta");
    let dst_meta = fresh.get_map("meta");
    if let LoroValue::Map(m) = src_meta.get_deep_value() {
        for (k, v) in m.iter() {
            dst_meta.insert(k, v.clone()).map_err(loro)?;
        }
    }
    let st = tree(doc);
    let dt = tree(&fresh);
    fn copy(st: &LoroTree, dt: &LoroTree, src: TreeID, dst_parent: Option<TreeID>, index: usize) -> Result<()> {
        let node = dt.create_at(dst_parent, index).map_err(loro)?;
        let dm = dt.get_meta(node).map_err(loro)?;
        if let Ok(sm) = st.get_meta(src) {
            if let LoroValue::Map(m) = sm.get_deep_value() {
                for (k, v) in m.iter() {
                    dm.insert(k, v.clone()).map_err(loro)?;
                }
            }
        }
        for (i, c) in st.children(Some(src)).unwrap_or_default().into_iter().enumerate() {
            copy(st, dt, c, Some(node), i)?;
        }
        Ok(())
    }
    for (i, r) in st.roots().into_iter().enumerate() {
        copy(&st, &dt, r, None, i)?;
    }
    fresh.commit();
    Ok(fresh)
}

/// Flatten into id → (parent id, title, done, notes) for diffing.
fn flat(doc: &LoroDoc) -> BTreeMap<String, (Option<String>, String, bool, String)> {
    fn walk(v: &TaskView, parent: Option<String>, out: &mut BTreeMap<String, (Option<String>, String, bool, String)>) {
        out.insert(v.id.clone(), (parent, v.title.clone(), v.done, v.notes.clone()));
        for c in &v.children {
            walk(c, Some(v.id.clone()), out);
        }
    }
    let mut out = BTreeMap::new();
    for t in tasks(doc) {
        walk(&t, None, &mut out);
    }
    out
}

/// Rebase (D24): re-apply the changes that turned `base` into `local` onto `target`, as new
/// ops. Used by a device that had unsynced edits when a document epoch arrived.
pub fn rebase(base: &LoroDoc, local: &LoroDoc, target: &LoroDoc) -> Result<()> {
    let b = flat(base);
    let l = flat(local);
    if list_name(local) != list_name(base) {
        rename_list(target, &list_name(local))?;
    }
    // Added tasks (parents before children: BTreeMap order isn't topological, so loop).
    let mut pending: Vec<&String> = l.keys().filter(|k| !b.contains_key(*k)).collect();
    let mut progress = true;
    while !pending.is_empty() && progress {
        progress = false;
        pending.retain(|id| {
            let (parent, title, done, notes) = &l[*id];
            let parent_ok = parent.as_ref().map(|p| find(target, p).is_ok()).unwrap_or(true);
            if !parent_ok {
                return true;
            }
            let t = tree(target);
            let pn = parent.as_ref().and_then(|p| find(target, p).ok());
            let index = t.children_num(pn).unwrap_or(0);
            if let Ok(node) = t.create_at(pn, index) {
                if let Ok(meta) = t.get_meta(node) {
                    let _ = meta.insert("id", id.as_str());
                    let _ = meta.insert("title", title.as_str());
                    let _ = meta.insert("done", *done);
                    if !notes.is_empty() {
                        let _ = meta.insert("notes", notes.as_str());
                    }
                }
            }
            progress = true;
            false
        });
    }
    // Modified and deleted tasks.
    for (id, (bp, bt, bd, bn)) in &b {
        match l.get(id) {
            None => {
                let _ = delete_task(target, id);
            }
            Some((lp, lt, ld, ln)) => {
                if lt != bt {
                    let _ = set_title(target, id, lt);
                }
                if ld != bd {
                    let _ = set_done(target, id, *ld);
                }
                if ln != bn {
                    let _ = set_notes(target, id, ln);
                }
                if lp != bp {
                    let _ = move_task(target, id, lp.as_deref(), usize::MAX);
                }
            }
        }
    }
    target.commit();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use loro::ExportMode;

    fn doc() -> LoroDoc {
        let d = LoroDoc::new();
        d.set_peer_id(1).unwrap();
        init_list(&d, "Groceries").unwrap();
        d
    }

    #[test]
    fn crud_and_nesting() {
        let d = doc();
        let milk = add_task(&d, None, "milk").unwrap();
        let bread = add_task(&d, None, "bread").unwrap();
        let wg = add_task(&d, Some(&bread), "whole grain").unwrap();
        set_done(&d, &milk, true).unwrap();
        set_title(&d, &bread, "bread!").unwrap();
        let v = tasks(&d);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].title, "milk");
        assert!(v[0].done);
        assert_eq!(v[1].children[0].id, wg);
        delete_task(&d, &milk).unwrap();
        assert_eq!(tasks(&d).len(), 1);
        assert_eq!(list_name(&d), "Groceries");
    }

    #[test]
    fn rebuild_keeps_state_drops_tombstones() {
        let d = doc();
        for i in 0..300 {
            let id = add_task(&d, None, &format!("task {i}")).unwrap();
            if i % 10 != 0 {
                delete_task(&d, &id).unwrap();
            }
        }
        let fresh = rebuild(&d, 2).unwrap();
        assert_eq!(normalized(&fresh), normalized(&d));
        let old = d.export(ExportMode::Snapshot).unwrap().len();
        let new = fresh.export(ExportMode::Snapshot).unwrap().len();
        assert!(new * 3 < old, "rebuild should shrink the document: {old} -> {new}");
    }

    #[test]
    fn rebase_applies_local_changes() {
        let base = doc();
        let a = add_task(&base, None, "a").unwrap();
        let b = add_task(&base, None, "b").unwrap();
        let local = base.fork();
        local.set_peer_id(3).unwrap();
        set_done(&local, &a, true).unwrap();
        delete_task(&local, &b).unwrap();
        let c = add_task(&local, None, "c").unwrap();
        add_task(&local, Some(&c), "c-child").unwrap();
        let target = rebuild(&base, 2).unwrap();
        target.set_peer_id(3).unwrap();
        rebase(&base, &local, &target).unwrap();
        assert_eq!(normalized(&target), normalized(&local));
    }
}

/// Document semantics for the engine's document epochs (D24).
pub struct TaskSemantics;

impl kt_engine::replica::DocSemantics for TaskSemantics {
    fn normalized(&self, doc: &LoroDoc) -> Vec<u8> {
        normalized(doc)
    }
    fn rebuild(&self, doc: &LoroDoc, peer: u64) -> kt_engine::Result<LoroDoc> {
        rebuild(doc, peer).map_err(|e| kt_engine::Error::Invalid(e.to_string()))
    }
    fn rebase(&self, base: &LoroDoc, local: &LoroDoc, target: &LoroDoc) -> kt_engine::Result<()> {
        rebase(base, local, target).map_err(|e| kt_engine::Error::Invalid(e.to_string()))
    }
}
