fn main() {
    println!("cargo:rerun-if-env-changed=ANTHRO_BRIDGE_CHANNEL");
    println!("cargo:rerun-if-env-changed=PCT_UPDATER_BUILD_MODE");
    println!("cargo:rerun-if-env-changed=PCT_UPDATER_PUBLIC_KEY");
    tauri_build::build()
}
