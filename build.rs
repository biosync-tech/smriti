//! Ensure `smriti-web/dist` exists so the `webui` feature can compile.
//!
//! `include_dir!` requires the directory at rustc time. The real SPA is
//! produced by `cd smriti-web && npm run build` and is gitignored. CI and
//! `--all-features` builds get a stub page instead of a missing-path error.

fn main() {
    let dist = std::path::Path::new("smriti-web/dist");
    let index = dist.join("index.html");
    if !index.exists() {
        if let Err(e) = std::fs::create_dir_all(dist) {
            panic!("could not create smriti-web/dist: {e}");
        }
        if let Err(e) = std::fs::write(
            &index,
            "<!doctype html><html><head><title>smriti</title></head>\
             <body>Web UI not built. Run: cd smriti-web && npm run build</body></html>\n",
        ) {
            panic!("could not write stub smriti-web/dist/index.html: {e}");
        }
        println!("cargo:warning=smriti-web/dist missing; wrote a compile stub");
    }
    println!("cargo:rerun-if-changed=smriti-web/dist/index.html");
}
