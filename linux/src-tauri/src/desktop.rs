#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnvironment {
    Hyprland,
    Niri,
    Plasma,
    Gnome,
    Other,
}

impl DesktopEnvironment {
    pub fn identifier(self) -> &'static str {
        match self {
            Self::Hyprland => "hyprland",
            Self::Niri => "niri",
            Self::Plasma => "plasma",
            Self::Gnome => "gnome",
            Self::Other => "other",
        }
    }

    // Tiling compositors own window movement/close shortcuts and do not need
    // a duplicate client-side title bar.  GNOME and Plasma retain normal
    // platform decorations and their native controls.
    pub fn prefers_borderless_window(self) -> bool {
        matches!(self, Self::Hyprland | Self::Niri)
    }
}

pub fn detect() -> DesktopEnvironment {
    let desktop = [
        std::env::var("XDG_CURRENT_DESKTOP").ok(),
        std::env::var("XDG_SESSION_DESKTOP").ok(),
        std::env::var("DESKTOP_SESSION").ok(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(":")
    .to_ascii_lowercase();

    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        || desktop.contains("hyprland")
    {
        DesktopEnvironment::Hyprland
    } else if std::env::var_os("NIRI_SOCKET").is_some() || desktop.contains("niri") {
        DesktopEnvironment::Niri
    } else if std::env::var_os("KDE_FULL_SESSION").is_some()
        || desktop.contains("plasma")
        || desktop.contains("kde")
    {
        DesktopEnvironment::Plasma
    } else if desktop.contains("gnome") {
        DesktopEnvironment::Gnome
    } else {
        DesktopEnvironment::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiling_desktops_request_borderless_windows() {
        assert!(DesktopEnvironment::Hyprland.prefers_borderless_window());
        assert!(DesktopEnvironment::Niri.prefers_borderless_window());
        assert!(!DesktopEnvironment::Plasma.prefers_borderless_window());
        assert!(!DesktopEnvironment::Gnome.prefers_borderless_window());
    }
}
