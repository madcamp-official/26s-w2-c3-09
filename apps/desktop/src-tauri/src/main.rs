#[cfg(feature = "tauri-commands")]
fn main() {
    mousekeeper_desktop::run();
}

#[cfg(not(feature = "tauri-commands"))]
fn main() {}
