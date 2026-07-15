pub const MAIN_WINDOW_LABEL: &str = "main";
pub const HOUSE_OVERLAY_WINDOW_LABEL: &str = "house-overlay";

#[cfg(feature = "tauri-commands")]
pub fn require_main_window(window: &tauri::Window) -> Result<(), String> {
    require_main_window_label(window.label())
}

#[cfg(feature = "tauri-commands")]
pub fn require_file_manager_window(window: &tauri::Window) -> Result<(), String> {
    require_file_manager_window_label(window.label())
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

pub fn require_file_manager_window_label(label: &str) -> Result<(), String> {
    if label == MAIN_WINDOW_LABEL || label == HOUSE_OVERLAY_WINDOW_LABEL {
        Ok(())
    } else {
        Err(format!(
            "FORBIDDEN_WINDOW: command is only available from the {MAIN_WINDOW_LABEL} or {HOUSE_OVERLAY_WINDOW_LABEL} window"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{require_file_manager_window_label, require_main_window_label};

    #[test]
    fn accepts_main_window() {
        require_main_window_label("main").expect("main window");
    }

    #[test]
    fn rejects_character_overlay_window() {
        let error = require_main_window_label("character-overlay").expect_err("overlay blocked");

        assert!(error.contains("FORBIDDEN_WINDOW"));
    }

    #[test]
    fn file_manager_accepts_main_and_house_overlay() {
        require_file_manager_window_label("main").expect("main window");
        require_file_manager_window_label("house-overlay").expect("house overlay manager");
    }

    #[test]
    fn file_manager_rejects_character_and_chat_overlay() {
        require_file_manager_window_label("character-overlay").expect_err("character blocked");
        require_file_manager_window_label("chat-overlay").expect_err("chat blocked");
    }
}
