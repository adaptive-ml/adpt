use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=THIRD_PARTY_LICENSES.txt");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src = Path::new(&manifest_dir).join("THIRD_PARTY_LICENSES.txt");
    let dst = Path::new(&env::var("OUT_DIR").unwrap()).join("third_party_licenses.txt");

    let content = fs::read_to_string(&src).unwrap_or_else(|_| {
        "Third-party license notices are generated at release build time and are not \
         present in this build.\n\nDownload the full THIRD_PARTY_LICENSES.txt for a \
         release from https://github.com/adaptive-ml/adpt/releases, or run \
         scripts/generate-third-party-licenses.sh from a checkout to produce it.\n"
            .to_string()
    });

    fs::write(&dst, content).unwrap();
}
