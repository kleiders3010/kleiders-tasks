fn main() {
    let path = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    eprintln!("importing {} bytes", bytes.len());
    let d = loro::LoroDoc::new();
    let r = d.import(&bytes);
    eprintln!("import returned: {:?}", r.as_ref().map(|_| ()));
    let _ = d.get_deep_value();
    eprintln!("deep value ok");
    let _ = d.export(loro::ExportMode::Snapshot);
    eprintln!("export ok");
}
