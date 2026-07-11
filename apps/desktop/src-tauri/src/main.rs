#[cfg(feature = "tauri-commands")]
fn main() {
    housemouse_desktop::run();
}

#[cfg(not(feature = "tauri-commands"))]
fn main() {}
