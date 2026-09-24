//! Tiny `--key value` / `--flag` parser (repeatable keys).
use std::collections::HashMap;
use std::str::FromStr;

pub struct Args {
    pub cmd: String,
    kv: HashMap<String, Vec<String>>,
}

impl Args {
    pub fn parse() -> Self {
        let mut it = std::env::args().skip(1).peekable();
        let cmd = it.next().unwrap_or_default();
        let mut kv: HashMap<String, Vec<String>> = HashMap::new();
        while let Some(k) = it.next() {
            let k = k.trim_start_matches("--").to_string();
            let v = match it.peek() {
                Some(n) if !n.starts_with("--") => it.next().unwrap(),
                _ => "true".to_string(),
            };
            kv.entry(k).or_default().push(v);
        }
        Self { cmd, kv }
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.kv.get(k).and_then(|v| v.last()).map(|s| s.as_str())
    }
    pub fn all(&self, k: &str) -> Vec<String> {
        self.kv.get(k).cloned().unwrap_or_default()
    }
    pub fn flag(&self, k: &str) -> bool {
        matches!(self.get(k), Some("true") | Some("1") | Some("yes"))
    }
    pub fn or<T: FromStr>(&self, k: &str, d: T) -> T {
        self.get(k).and_then(|s| s.parse().ok()).unwrap_or(d)
    }
    pub fn req<T: FromStr>(&self, k: &str) -> anyhow::Result<T>
    where
        T::Err: std::fmt::Display,
    {
        let s = self.get(k).ok_or_else(|| anyhow::anyhow!("missing --{k}"))?;
        s.parse().map_err(|e| anyhow::anyhow!("bad --{k} {s:?}: {e}"))
    }
}
