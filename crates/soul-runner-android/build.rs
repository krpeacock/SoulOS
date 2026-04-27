// Walk the assets directory at build time and emit every subdirectory path
// as a comma-separated env var. bootstrap() uses this list to extract each
// subdirectory explicitly, because AAssetDir_getNextFileName only returns
// file names — it never yields subdirectory names.
fn main() {
    let assets = std::path::PathBuf::from("../../assets");
    let mut dirs: Vec<String> = Vec::new();
    collect_subdirs(&assets, &assets, &mut dirs);
    dirs.sort();
    println!("cargo:rustc-env=SOUL_ASSET_DIRS={}", dirs.join(","));
    println!("cargo:rerun-if-changed=../../assets");
}

fn collect_subdirs(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
            collect_subdirs(root, &path, out);
        }
    }
}
