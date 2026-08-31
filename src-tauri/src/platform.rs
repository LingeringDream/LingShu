use serde::Serialize;

/// Runtime capability map consumed by the shared React frontend.
///
/// Keep this conservative: a capability is true only when the current desktop
/// build has a working implementation, not merely when Tauri can compile for
/// that operating system. This prevents Windows/Linux builds from exposing
/// macOS-only actions and failing after the user clicks them.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    platform: &'static str,
    is_desktop: bool,
    calendar_provider: &'static str,
    screen_reader: bool,
    application_automation: bool,
    url_opener: bool,
    path_opener: bool,
    file_reader: bool,
    pdf_reader: bool,
    desktop_pet: bool,
}

impl PlatformCapabilities {
    fn current() -> Self {
        let is_macos = cfg!(target_os = "macos");
        Self {
            platform: current_platform_name(),
            is_desktop: true,
            calendar_provider: if is_macos { "eventkit" } else { "ics" },
            screen_reader: is_macos,
            application_automation: is_macos,
            url_opener: true,
            path_opener: true,
            file_reader: true,
            pdf_reader: true,
            desktop_pet: true,
        }
    }
}

fn current_platform_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

#[tauri::command]
pub fn get_platform_capabilities() -> PlatformCapabilities {
    PlatformCapabilities::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_compiled_platform() {
        let capabilities = PlatformCapabilities::current();
        assert_eq!(capabilities.platform, current_platform_name());
        assert!(capabilities.is_desktop);
        assert!(capabilities.desktop_pet);
        assert!(capabilities.url_opener);
        assert!(capabilities.path_opener);
        assert!(capabilities.file_reader);
        assert!(capabilities.pdf_reader);
    }

    #[test]
    fn uses_ics_outside_macos() {
        let capabilities = PlatformCapabilities::current();
        if cfg!(target_os = "macos") {
            assert_eq!(capabilities.calendar_provider, "eventkit");
            assert!(capabilities.screen_reader);
        } else {
            assert_eq!(capabilities.calendar_provider, "ics");
            assert!(!capabilities.screen_reader);
        }
    }
}
