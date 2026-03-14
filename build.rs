use embed_manifest::{embed_manifest, new_manifest};
use embed_manifest::manifest::ExecutionLevel;

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = new_manifest("YourApp.Name")
            .requested_execution_level(ExecutionLevel::RequireAdministrator);

        embed_manifest(manifest)
            .expect("unable to embed manifest");
    }

    println!("cargo:rerun-if-changed=build.rs");
}