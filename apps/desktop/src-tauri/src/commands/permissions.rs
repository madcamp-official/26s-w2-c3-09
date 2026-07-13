pub const MAIN_WINDOW_LABEL: &str = "main";

#[cfg(feature = "tauri-commands")]
pub fn require_main_window(window: &tauri::Window) -> Result<(), String> {
    require_main_window_label(window.label())
}

pub fn require_main_window_label(label: &str) -> Result<(), String> {
    if label == MAIN_WINDOW_LABEL {
        Ok(())
    } else {
        Err(format!(
            "FORBIDDEN_WINDOW: command is only available from the {MAIN_WINDOW_LABEL} window"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::require_main_window_label;

    #[test]
    fn accepts_main_window() {
        require_main_window_label("main").expect("main window");
    }

    #[test]
    fn rejects_character_overlay_window() {
        let error = require_main_window_label("character-overlay").expect_err("overlay blocked");

        assert!(error.contains("FORBIDDEN_WINDOW"));
    }
}
