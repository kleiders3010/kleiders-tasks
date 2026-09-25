//! kt-tasks: the task-list domain on top of kt-engine (thin, app-specific).
//!
//! Schema of one list document:
//! - map `meta`: `name` (string), `hue` (i64 0–360 or absent), `icon` (string or absent)
//! - tree `tasks`: each node's meta map holds
//!   - `id`: stable task ID (random hex); survives document epochs, which recreate tree nodes
//!   - `title` (string), `done` (bool), `notes` (string): inline text up to the attachment
//!     threshold; longer notes are a text attachment, stored as [`BLOB_MARK`] + blob
//!     reference + newline + a preview, in this one field so every edit, undo and merge
//!     moves reference and preview together
//!
//! Task order is the tree's fractional-index order. Unknown fields are preserved
//! (principle 8): the domain never deletes keys it doesn't know.

use std::collections::BTreeMap;

pub mod app;
pub mod appearance;
pub mod export;
pub mod history;

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
    /// A timed reminder (ms since the epoch), shared like any field.
    pub remind_ms: Option<u64>,
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

/// One task (with its subtasks), if it exists.
pub(crate) fn find_view(doc: &LoroDoc, task_id: &str) -> Option<TaskView> {
    let n = find(doc, task_id).ok()?;
    view_of(&tree(doc), n)
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
    meta.insert("created_ms", now_ms() as i64).map_err(loro)?;
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
    let meta = tree(doc).get_meta(n).map_err(loro)?;
    meta.insert("done", done).map_err(loro)?;
    if done {
        meta.insert("done_ms", now_ms() as i64).map_err(loro)?;
    }
    doc.commit();
    Ok(())
}

/// Set a task's notes: inline text, or an attachment's notes value ([`attachment_notes`]).
/// Text above the attachment threshold (D4) goes through `App::set_notes`, which makes the
/// blob; here it's refused, never silently cut.
pub fn set_notes(doc: &LoroDoc, task: &str, notes: &str) -> Result<()> {
    let value = if notes.starts_with(BLOB_MARK) {
        match notes_of(notes) {
            Notes::Attachment { .. } => notes.to_string(),
            Notes::Inline(_) => return Err(Error::Invalid("not a valid attachment".into())),
        }
    } else if notes.len() > TEXT_ATTACHMENT_THRESHOLD {
        return Err(Error::Invalid("notes this long go through App::set_notes (a text attachment)".into()));
    } else {
        notes.replace('\u{0}', "") // the marker can't be typed in
    };
    let n = find(doc, task)?;
    tree(doc).get_meta(n).map_err(loro)?.insert("notes", value.as_str()).map_err(loro)?;
    doc.commit();
    Ok(())
}

/// Starts a notes value that is a text attachment.
pub const BLOB_MARK: &str = "\u{0}blob:";
/// Characters of an attachment kept inline as its preview (lists, exports, widgets).
pub const PREVIEW_CHARS: usize = 2000;

/// A task's notes, as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notes<'a> {
    Inline(&'a str),
    Attachment { blob: kt_engine::blobs::BlobRef, preview: &'a str },
}

/// Read a notes value. A malformed attachment reads as no notes, never as the marker text.
pub fn notes_of(raw: &str) -> Notes<'_> {
    let Some(rest) = raw.strip_prefix(BLOB_MARK) else { return Notes::Inline(raw) };
    let (r, preview) = rest.split_once('\n').unwrap_or((rest, ""));
    match kt_engine::blobs::BlobRef::parse(r) {
        Ok(blob) => Notes::Attachment { blob, preview },
        Err(_) => Notes::Inline(""),
    }
}

/// The notes value for a text attachment of `full`.
pub fn attachment_notes(blob: &kt_engine::blobs::BlobRef, full: &str) -> String {
    let preview: String = full.chars().take(PREVIEW_CHARS).collect();
    format!("{BLOB_MARK}{}\n{preview}", blob.encode())
}

impl TaskView {
    /// Notes to show without the attachment: inline notes, or an attachment's preview.
    pub fn notes_preview(&self) -> &str {
        match notes_of(&self.notes) {
            Notes::Inline(t) => t,
            Notes::Attachment { preview, .. } => preview,
        }
    }

    pub fn attachment(&self) -> Option<kt_engine::blobs::BlobRef> {
        match notes_of(&self.notes) {
            Notes::Attachment { blob, .. } => Some(blob),
            Notes::Inline(_) => None,
        }
    }
}

/// Set or clear a task's reminder (ms since the epoch).
pub fn set_reminder(doc: &LoroDoc, task: &str, at_ms: Option<u64>) -> Result<()> {
    let n = find(doc, task)?;
    let meta = tree(doc).get_meta(n).map_err(loro)?;
    match at_ms {
        Some(t) => meta.insert("remind_ms", t as i64).map_err(loro)?,
        None => meta.delete("remind_ms").map_err(loro)?,
    }
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
        remind_ms: match meta.get("remind_ms") {
            Some(ValueOrContainer::Value(LoroValue::I64(v))) if v > 0 => Some(v as u64),
            _ => None,
        },
        id: get_str(&meta, "id").unwrap_or_default(),
        title: get_str(&meta, "title").unwrap_or_default(),
        done: get_bool(&meta, "done").unwrap_or(false),
        notes: get_str(&meta, "notes").unwrap_or_default(),
        children,
    })
}

/// All tasks in display order, nested.
/// Who did what when, per task (for notifications about others' changes). Times are as
/// the editing device stated them; peers are Loro peer IDs (see `kt_engine::peer_id_for`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskActivity {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub created_ms: u64,
    pub created_by: u64,
    pub done_ms: u64,
    pub done_by: Option<u64>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn get_ms(m: &LoroMap, k: &str) -> u64 {
    match m.get(k) {
        Some(ValueOrContainer::Value(LoroValue::I64(v))) => v.max(0) as u64,
        _ => 0,
    }
}

/// Every live task (any depth) with its authorship.
pub fn activity(doc: &LoroDoc) -> Vec<TaskActivity> {
    let t = tree(doc);
    let mut out = Vec::new();
    for n in t.nodes() {
        if t.is_node_deleted(&n).unwrap_or(true) {
            continue;
        }
        let Ok(meta) = t.get_meta(n) else { continue };
        let Some(id) = get_str(&meta, "id") else { continue };
        let done = matches!(meta.get("done"), Some(ValueOrContainer::Value(LoroValue::Bool(true))));
        out.push(TaskActivity {
            id,
            title: get_str(&meta, "title").unwrap_or_default(),
            done,
            created_ms: get_ms(&meta, "created_ms"),
            created_by: n.peer,
            done_ms: get_ms(&meta, "done_ms"),
            done_by: meta.get_last_editor("done"),
        });
    }
    out
}

pub fn tasks(doc: &LoroDoc) -> Vec<TaskView> {
    let t = tree(doc);
    t.roots().into_iter().filter_map(|n| view_of(&t, n)).collect()
}

/// Canonical, ID-stable representation of a list's live state. Two documents with equal
/// normalized states show the same list, even if their Loro internals differ.
pub fn normalized(doc: &LoroDoc) -> Vec<u8> {
    fn walk(t: &LoroTree, n: TreeID, depth: usize, out: &mut String) {
        if let Ok(meta) = t.get_meta(n) {
            out.push_str(&format!("{depth}|{}\n", canonical(&meta.get_deep_value())));
        }
        for c in t.children(Some(n)).unwrap_or_default() {
            walk(t, c, depth + 1, out);
        }
    }
    let mut out = String::new();
    // Every root map (list meta; group docs' lists, people, ...; profile, index), not just
    // the task tree: a rebuild that dropped any of them must not verify.
    for (name, v) in root_maps(doc) {
        out.push_str(&format!("map {name}|{}\n", canonical(&v)));
    }
    let t = tree(doc);
    for r in t.roots() {
        walk(&t, r, 0, &mut out);
    }
    out.into_bytes()
}

/// A value as text with map keys sorted (Loro's maps iterate in hash order).
fn canonical(v: &LoroValue) -> String {
    match v {
        LoroValue::Map(m) => {
            let sorted: BTreeMap<&String, &LoroValue> = m.iter().collect();
            let inner: Vec<String> = sorted.into_iter().map(|(k, v)| format!("{k:?}:{}", canonical(v))).collect();
            format!("{{{}}}", inner.join(","))
        }
        LoroValue::List(l) => format!("[{}]", l.iter().map(canonical).collect::<Vec<_>>().join(",")),
        other => format!("{other:?}"),
    }
}

/// The document's root maps by name (everything except the task tree).
fn root_maps(doc: &LoroDoc) -> BTreeMap<String, LoroValue> {
    match doc.get_deep_value() {
        LoroValue::Map(m) => m.iter().filter(|(k, v)| k.as_str() != "tasks" && matches!(v, LoroValue::Map(_))).map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => BTreeMap::new(),
    }
}

/// Write `v` under `key` in `map`, recreating nested maps and lists as containers.
fn put_deep(map: &LoroMap, key: &str, v: &LoroValue) -> Result<()> {
    match v {
        LoroValue::Map(m) => {
            let child = map.insert_container(key, LoroMap::new()).map_err(loro)?;
            for (k, x) in m.iter() {
                put_deep(&child, k, x)?;
            }
        }
        LoroValue::List(l) => {
            let child = map.insert_container(key, loro::LoroList::new()).map_err(loro)?;
            for x in l.iter() {
                child.push(x.clone()).map_err(loro)?;
            }
        }
        other => {
            map.insert(key, other.clone()).map_err(loro)?;
        }
    }
    Ok(())
}

/// Build a fresh document with the same live state (document epoch, D24): no tombstones,
/// no history. Stable task IDs, order and unknown fields are preserved.
pub fn rebuild(doc: &LoroDoc, peer: u64) -> Result<LoroDoc> {
    let fresh = LoroDoc::new();
    fresh.set_peer_id(peer).map_err(loro)?;
    // Every root map (a group doc's lists and people, a profile, an index...), not just meta.
    for (name, v) in root_maps(doc) {
        if let LoroValue::Map(m) = v {
            let dst = fresh.get_map(name.as_str());
            for (k, x) in m.iter() {
                put_deep(&dst, k, x)?;
            }
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
    // Root maps (list meta, group lists/people, profile, index): keys changed locally.
    let (bm, lm) = (root_maps(base), root_maps(local));
    for name in bm.keys().chain(lm.keys()).collect::<std::collections::BTreeSet<_>>() {
        let empty = LoroValue::Map(Default::default());
        let (LoroValue::Map(bv), LoroValue::Map(lv)) = (bm.get(name).unwrap_or(&empty), lm.get(name).unwrap_or(&empty)) else { continue };
        let dst = target.get_map(name.as_str());
        for (k, v) in lv.iter() {
            if bv.get(k) != Some(v) {
                put_deep(&dst, k, v)?;
            }
        }
        for k in bv.keys().filter(|k| !lv.contains_key(*k)) {
            dst.delete(k).map_err(loro)?;
        }
    }
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
                    // Every field as it is locally (times included), not just the known ones.
                    let local_meta = find(local, id).ok().and_then(|n| tree(local).get_meta(n).ok());
                    match local_meta.map(|m| m.get_deep_value()) {
                        Some(LoroValue::Map(m)) => {
                            for (k, v) in m.iter() {
                                let _ = meta.insert(k, v.clone());
                            }
                        }
                        _ => {
                            let _ = meta.insert("id", id.as_str());
                            let _ = meta.insert("title", title.as_str());
                            let _ = meta.insert("done", *done);
                            if !notes.is_empty() {
                                let _ = meta.insert("notes", notes.as_str());
                            }
                        }
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
                    // Keep when it was done locally, not when it was rebased.
                    if let (Ok(ln), Ok(tn)) = (find(local, id), find(target, id)) {
                        if let (Ok(lm), Ok(tm)) = (tree(local).get_meta(ln), tree(target).get_meta(tn)) {
                            if let Some(ValueOrContainer::Value(v)) = lm.get("done_ms") {
                                let _ = tm.insert("done_ms", v);
                                target.commit();
                            }
                        }
                    }
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
    fn compaction_keeps_every_map() {
        // A group document: meta, a list index and people (not just a task tree).
        let d = doc();
        d.get_map("meta").insert("kind", "group").unwrap();
        d.get_map("lists").insert("aa11", true).unwrap();
        d.get_map("lists").insert("bb22", true).unwrap();
        d.get_map("people").insert("dev1", "Alex").unwrap();
        d.commit();
        let fresh = rebuild(&d, 9).unwrap();
        assert_eq!(normalized(&fresh), normalized(&d), "nothing dropped");
        assert_eq!(fresh.get_map("people").get_deep_value(), d.get_map("people").get_deep_value());
        // Same content, other insertion order: equal.
        let other = doc();
        other.get_map("people").insert("dev1", "Alex").unwrap();
        other.get_map("lists").insert("bb22", true).unwrap();
        other.get_map("lists").insert("aa11", true).unwrap();
        other.get_map("meta").insert("kind", "group").unwrap();
        other.commit();
        assert_eq!(normalized(&other), normalized(&d));
        // A dropped map doesn't verify.
        let lossy = doc();
        lossy.get_map("meta").insert("kind", "group").unwrap();
        lossy.commit();
        assert_ne!(normalized(&lossy), normalized(&d));
        // Rebase carries map edits made before the switch.
        let local = d.fork();
        local.set_peer_id(3).unwrap();
        local.get_map("people").insert("dev2", "Sam").unwrap();
        local.get_map("lists").delete("bb22").unwrap();
        local.commit();
        let target = rebuild(&d, 2).unwrap();
        target.set_peer_id(3).unwrap();
        rebase(&d, &local, &target).unwrap();
        assert_eq!(normalized(&target), normalized(&local));
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
