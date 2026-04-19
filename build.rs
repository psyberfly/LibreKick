use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR"));
    let seed_dir = manifest_dir.join("src/assets/seed_patches");

    println!("cargo:rerun-if-changed={}", seed_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Missing OUT_DIR"));
    let generated_path = out_dir.join("embedded_seed_patches.rs");

    let mut entries = Vec::new();
    if seed_dir.exists() {
        let read_dir = fs::read_dir(&seed_dir).expect("Failed to read src/assets/seed_patches");
        for entry in read_dir {
            let entry = entry.expect("Failed to read seed patch entry");
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !filename.ends_with(".librekick_patch") {
                continue;
            }

            entries.push((filename.to_owned(), path));
        }
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut generated = String::from("pub static EMBEDDED_SEED_PATCHES: &[(&str, &str)] = &[\n");
    for (filename, path) in entries {
        let include_path = path.to_string_lossy().replace('\\', "/");
        generated.push_str(&format!(
            "    (\"{filename}\", include_str!(r#\"{include_path}\"#)),\n"
        ));
    }
    generated.push_str("];\n");

    fs::write(&generated_path, generated).expect("Failed to write embedded seed patch index");
}
