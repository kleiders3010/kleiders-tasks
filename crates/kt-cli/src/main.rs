//! kt: command-line prototype of Kleiders Tasks (M1). Headless; one "device" per --home dir.
//!
//! Devices exchange messages through files (a manual transport) until M2 adds real ones:
//!
//! ```text
//! kt --home A init                      kt --home B init
//! kt --home A new-group Family          kt --home B whoami        -> <B's device key>
//! kt --home A new-list Groceries --group Family
//! kt --home A add Groceries milk
//! kt --home A share Family <B key> --out join.bin
//!                                       kt --home B join join.bin
//!                                       kt --home B add Groceries eggs
//!                                       kt --home B send b.msgs
//! kt --home A recv b.msgs
//! kt --home A tasks Groceries           -> milk, eggs
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use kt_engine::crypto::DevicePublic;
use kt_engine::sandbox::SandboxConfig;
use kt_engine::{storage, GroupId};
use kt_tasks::app::App;
use kt_tasks::TaskView;

fn usage() -> &'static str {
    "usage: kt --home DIR <command>
  init                         create this device (identity + personal group)
  whoami                       print this device's public key
  name [NEW]                   show or set your display name
  new-group NAME               create a group (a circle of people; holds several lists)
  groups                       show groups
  new-list NAME [--group G]    create a list (default: your personal group = solo list)
  lists                        show lists by group
  move-list LIST GROUP         move a list into another group
  add LIST TITLE [--under TASK]  add a task (TASK = id prefix)
  tasks LIST                   show tasks
  done LIST TASK / undone LIST TASK
  rename LIST TASK TITLE
  rm LIST TASK
  share GROUP DEVICE_KEY --out FILE  add a device to a group; write its join bundle
  join FILE                    join a group from a bundle
  send FILE                    write this device's pending messages to FILE
  recv FILE                    apply messages from FILE
  epoch LIST                   propose a document epoch (compaction)
  info LIST                    generation, members, sizes"
}

fn main() {
    if std::env::args().any(|a| a == "--kt-import-worker") {
        kt_engine::sandbox::worker_main();
    }
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn open(home: &Path) -> Result<App> {
    let worker = std::env::current_exe().context("locating own executable for the import sandbox")?;
    let sandbox = SandboxConfig { worker, timeout: Duration::from_secs(20) };
    App::open(&home.join("identity"), &home.join("data"), sandbox).map_err(|e| anyhow!("{e}"))
}

fn device_key(p: &DevicePublic) -> String {
    format!("{}{}", hex::encode(p.sign), hex::encode(p.kex))
}

fn parse_device_key(s: &str) -> Result<DevicePublic> {
    let b = hex::decode(s.trim()).context("device key must be hex")?;
    if b.len() != 64 {
        bail!("device key must be 64 bytes (128 hex chars)");
    }
    let mut sign = [0u8; 32];
    let mut kex = [0u8; 32];
    sign.copy_from_slice(&b[..32]);
    kex.copy_from_slice(&b[32..]);
    Ok(DevicePublic { sign, kex })
}

fn write_messages(path: &Path, msgs: &[Vec<u8>]) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    for m in msgs {
        storage::append_record(path, m).map_err(|e| anyhow!("{e}"))?;
    }
    Ok(())
}

fn read_messages(path: &Path) -> Result<Vec<Vec<u8>>> {
    storage::read_records(path).map_err(|e| anyhow!("{e}"))
}

fn find_task(tasks: &[TaskView], prefix: &str) -> Result<String> {
    fn walk<'a>(ts: &'a [TaskView], out: &mut Vec<&'a TaskView>) {
        for t in ts {
            out.push(t);
            walk(&t.children, out);
        }
    }
    let mut all = Vec::new();
    walk(tasks, &mut all);
    let m: Vec<&&TaskView> = all.iter().filter(|t| t.id.starts_with(prefix) || t.title == prefix).collect();
    match m.len() {
        1 => Ok(m[0].id.clone()),
        0 => bail!("no task matches '{prefix}'"),
        _ => bail!("'{prefix}' matches several tasks; use a longer id prefix"),
    }
}

fn print_tasks(ts: &[TaskView], depth: usize) {
    for t in ts {
        println!("{}[{}] {}  ({})", "  ".repeat(depth), if t.done { "x" } else { " " }, t.title, &t.id[..8]);
        print_tasks(&t.children, depth + 1);
    }
}

fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let home = match args.iter().position(|a| a == "--home") {
        Some(i) if i + 1 < args.len() => {
            let h = PathBuf::from(args[i + 1].clone());
            args.drain(i..=i + 1);
            h
        }
        _ => bail!("{}", usage()),
    };
    let take_flag = |args: &mut Vec<String>, flag: &str| -> Option<String> {
        let i = args.iter().position(|a| a == flag)?;
        if i + 1 < args.len() {
            let v = args[i + 1].clone();
            args.drain(i..=i + 1);
            Some(v)
        } else {
            None
        }
    };
    let out = take_flag(&mut args, "--out");
    let under = take_flag(&mut args, "--under");
    let group_flag = take_flag(&mut args, "--group");
    let cmd = args.first().cloned().unwrap_or_default();
    let arg = |i: usize| args.get(i).cloned().ok_or_else(|| anyhow!("{}", usage()));
    let mut app = open(&home)?;
    match cmd.as_str() {
        "init" => println!("device ready: {}", &device_key(&app.replica.device_public())[..16]),
        "whoami" => println!("{}", device_key(&app.replica.device_public())),
        "name" => match args.get(1) {
            Some(n) => app.set_display_name(n).map_err(|e| anyhow!("{e}"))?,
            None => println!("{}", app.display_name()),
        },
        "new-group" => {
            let g = app.create_group(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            println!("created group '{}' ({})", g.name, g.group.short());
        }
        "groups" => {
            for g in app.groups() {
                let members = app.replica.epochs(&g.group).and_then(|e| e.head().and_then(|h| e.recipients(&h).map(|r| r.len()))).unwrap_or(0);
                println!("{}  {}{}  ({} device{})", g.group.short(), g.name, if g.personal { " (personal)" } else { "" }, members, if members == 1 { "" } else { "s" });
            }
        }
        "new-list" => {
            let group = match &group_flag {
                Some(k) => Some(app.find_group(k).map_err(|e| anyhow!("{e}"))?.group),
                None => None,
            };
            let l = app.create_list(group, &arg(1)?).map_err(|e| anyhow!("{e}"))?;
            println!("created list '{}' in '{}' ({})", l.name, l.group_name, l.doc.short());
        }
        "lists" => {
            let mut current = String::new();
            for l in app.lists() {
                if l.group_name != current {
                    println!("{}:", l.group_name);
                    current = l.group_name.clone();
                }
                println!("  {}  ({})", l.name, l.doc.short());
            }
        }
        "move-list" => {
            let l = app.find_list(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            let g = app.find_group(&arg(2)?).map_err(|e| anyhow!("{e}"))?;
            let moved = app.move_list(&l, g.group).map_err(|e| anyhow!("{e}"))?;
            println!("moved '{}' to '{}'", moved.name, moved.group_name);
        }
        "add" => {
            let l = app.find_list(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            let title = arg(2)?;
            let parent = match &under {
                Some(p) => Some(find_task(&kt_tasks::tasks(app.replica.doc(&l.group, &l.doc).unwrap()), p)?),
                None => None,
            };
            let id = app.edit(&l, |d| kt_tasks::add_task(d, parent.as_deref(), &title)).map_err(|e| anyhow!("{e}"))?;
            println!("added {} ({})", title, &id[..8]);
        }
        "tasks" => {
            let l = app.find_list(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            println!("{}:", l.name);
            print_tasks(&kt_tasks::tasks(app.replica.doc(&l.group, &l.doc).unwrap()), 1);
        }
        "done" | "undone" | "rm" | "rename" => {
            let l = app.find_list(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            let t = find_task(&kt_tasks::tasks(app.replica.doc(&l.group, &l.doc).unwrap()), &arg(2)?)?;
            let res = match cmd.as_str() {
                "done" => app.edit(&l, |d| kt_tasks::set_done(d, &t, true)),
                "undone" => app.edit(&l, |d| kt_tasks::set_done(d, &t, false)),
                "rm" => app.edit(&l, |d| kt_tasks::delete_task(d, &t)),
                _ => {
                    let title = arg(3)?;
                    app.edit(&l, |d| kt_tasks::set_title(d, &t, &title))
                }
            };
            res.map_err(|e| anyhow!("{e}"))?;
        }
        "share" => {
            let g = app.find_group(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            let dev = parse_device_key(&arg(2)?)?;
            let out = PathBuf::from(out.ok_or_else(|| anyhow!("share needs --out FILE"))?);
            let mut msgs = vec![g.group.0.to_vec()];
            msgs.extend(app.share_group_with_device(g.group, dev).map_err(|e| anyhow!("{e}"))?);
            write_messages(&out, &msgs)?;
            println!("wrote join bundle for group '{}' to {}", g.name, out.display());
        }
        "join" => {
            let msgs = read_messages(Path::new(&arg(1)?))?;
            let (head, rest) = msgs.split_first().ok_or_else(|| anyhow!("empty bundle"))?;
            let g = GroupId(head.as_slice().try_into().map_err(|_| anyhow!("bad bundle"))?);
            app.join_from_bundle(g, rest).map_err(|e| anyhow!("{e}"))?;
            let name = app.groups().into_iter().find(|x| x.group == g).map(|x| x.name).unwrap_or_default();
            let lists: Vec<String> = app.lists().into_iter().filter(|l| l.group == g).map(|l| l.name).collect();
            println!("joined group '{}' ({}) with lists: {}", name, g.short(), lists.join(", "));
        }
        "send" => {
            let msgs = app.replica.outbox().map_err(|e| anyhow!("{e}"))?;
            write_messages(Path::new(&arg(1)?), &msgs)?;
            println!("wrote {} message(s)", msgs.len());
        }
        "recv" => {
            let msgs = read_messages(Path::new(&arg(1)?))?;
            let (mut ok, mut bad) = (0, 0);
            for m in &msgs {
                match app.replica.receive(m) {
                    Ok(()) => ok += 1,
                    Err(_) => bad += 1,
                }
            }
            let events = app.replica.take_events();
            let parked = events.iter().filter(|e| matches!(e, kt_engine::replica::Event::Parked)).count();
            println!("applied {ok} message(s), rejected {bad}, waiting {parked} (other groups or not yet decodable)");
            for e in events {
                match e {
                    kt_engine::replica::Event::Imported { .. } | kt_engine::replica::Event::EpochAccepted { .. } | kt_engine::replica::Event::Parked => {}
                    other => println!("  {other:?}"),
                }
            }
        }
        "epoch" => {
            let l = app.find_list(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            app.replica.propose_doc_epoch(&l.group, &l.doc).map_err(|e| anyhow!("{e}"))?;
            println!("proposed generation {}", app.replica.doc_generation(&l.group, &l.doc).unwrap_or(0));
        }
        "info" => {
            let l = app.find_list(&arg(1)?).map_err(|e| anyhow!("{e}"))?;
            let doc = app.replica.doc(&l.group, &l.doc).unwrap();
            let size = doc.export(loro::ExportMode::Snapshot).map(|s| s.len()).unwrap_or(0);
            println!("list '{}' in group '{}' ({}), doc {}", l.name, l.group_name, l.group.short(), l.doc.short());
            println!("generation {}  snapshot {} bytes", app.replica.doc_generation(&l.group, &l.doc).unwrap_or(0), size);
            if let Some(e) = app.replica.epochs(&l.group) {
                if let Some(h) = e.head() {
                    println!("epoch {}  devices {}", h.short(), e.recipients(&h).map(|r| r.len()).unwrap_or(0));
                }
            }
        }
        _ => bail!("{}", usage()),
    }
    Ok(())
}
