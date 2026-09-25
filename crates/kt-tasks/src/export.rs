//! Readable exports (DESIGN §11).

use crate::TaskView;

/// Markdown in the Obsidian Tasks format: `- [ ]` / `- [x]` items, subtasks indented with
/// tabs, notes as indented lines under their task. One-way: nothing reads this back.
pub fn markdown(list_name: &str, tasks: &[TaskView]) -> String {
    let mut out = format!("# {}\n\n", one_line(list_name));
    items(tasks, 0, &mut out);
    out
}

fn items(ts: &[TaskView], depth: usize, out: &mut String) {
    for t in ts {
        let indent = "\t".repeat(depth);
        out.push_str(&format!("{indent}- [{}] {}\n", if t.done { 'x' } else { ' ' }, one_line(&t.title)));
        for line in t.notes.lines().filter(|l| !l.trim().is_empty()) {
            out.push_str(&format!("{indent}  {}\n", line.trim_end()));
        }
        items(&t.children, depth + 1, out);
    }
}

/// Titles are one line in Markdown; an empty title would read as an empty checkbox.
fn one_line(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { "(untitled)".into() } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(title: &str, done: bool, notes: &str, children: Vec<TaskView>) -> TaskView {
        TaskView { id: String::new(), title: title.into(), done, notes: notes.into(), remind_ms: None, children }
    }

    #[test]
    fn obsidian_tasks_format() {
        let md = markdown("Groceries", &[t("milk", true, "", vec![]), t("party", false, "Saturday\n\nbring chairs", vec![t("cake\nchocolate", false, "", vec![])])]);
        assert_eq!(md, "# Groceries\n\n- [x] milk\n- [ ] party\n  Saturday\n  bring chairs\n\t- [ ] cake chocolate\n");
    }
}
