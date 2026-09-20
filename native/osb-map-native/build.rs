use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=MAPLIBRE_NATIVE_C_INSTALL_DIR");
    let prefix=env::var_os("MAPLIBRE_NATIVE_C_INSTALL_DIR").expect("Run tools/run-live.sh or source the path printed by tools/setup-maplibre.sh. Unpinned native snapshot downloads are not accepted.");
    let descriptor = PathBuf::from(prefix).join("share/maplibre-native-c/artifact.json");
    println!("cargo:rerun-if-changed={}", descriptor.display());
    let v: serde_json::Value =
        serde_json::from_slice(&fs::read(descriptor).expect("native artifact descriptor missing"))
            .expect("invalid native artifact descriptor");
    assert_eq!(
        v["gitSha"], "6f7998eec595560c0359ed033519cbab1f7c9aeb",
        "native C library and Rust wrapper must match the pinned revision"
    );
    assert_eq!(
        v["renderBackend"], "vulkan",
        "native basemap must use Vulkan"
    );
}
