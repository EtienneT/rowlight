//! Saved appearance overrides with native OS following and optional Omarchy.

use gpui_kit::{App, Global, WindowAppearance, rgb};
use gpui_omarchy::Theme;
use parquetviewer::settings::Appearance;

#[derive(Clone, Copy, PartialEq)]
enum Source {
    OperatingSystem,
    Omarchy,
    Fixed,
    #[cfg(debug_assertions)]
    Preview,
}

struct State {
    preference: Appearance,
    source: Source,
    omarchy_available: bool,
}
impl Global for State {}

pub fn ensure_initialized(preference: Appearance, cx: &mut App) {
    if cx.try_global::<State>().is_none() {
        apply(preference, cx);
    }
}

pub fn apply(preference: Appearance, cx: &mut App) {
    apply_with_omarchy(preference, omarchy_available(), cx);
}

fn apply_with_omarchy(preference: Appearance, omarchy_available: bool, cx: &mut App) {
    // Clearing the macOS application override restores the actual OS appearance
    // before reading it. Forced modes also match the native window chrome.
    let forced = match preference {
        Appearance::System => None,
        Appearance::Light => Some(WindowAppearance::Light),
        Appearance::Dark => Some(WindowAppearance::Dark),
    };
    cx.set_window_appearance(forced);
    let source = if preference == Appearance::System && omarchy_available {
        Theme::follow_system(cx);
        Source::Omarchy
    } else {
        native_palette(forced.unwrap_or_else(|| cx.window_appearance())).apply(cx);
        if preference == Appearance::System {
            Source::OperatingSystem
        } else {
            Source::Fixed
        }
    };
    cx.set_global(State {
        preference,
        source,
        omarchy_available,
    });
}

pub fn system_appearance_changed(appearance: WindowAppearance, cx: &mut App) {
    if cx.try_global::<State>().is_some_and(|state| {
        state.preference == Appearance::System && state.source == Source::OperatingSystem
    }) {
        native_palette(appearance).apply(cx);
    }
}

pub fn system_description(cx: &App) -> &'static str {
    if cx.global::<State>().omarchy_available {
        "System follows your Omarchy theme, including live palette changes."
    } else {
        "System follows your operating system’s light or dark appearance."
    }
}

#[cfg(debug_assertions)]
pub fn preview(theme: Theme, cx: &mut App) {
    cx.set_window_appearance(Some(match theme.appearance {
        gpui_kit::base::ThemeAppearance::Light => WindowAppearance::Light,
        gpui_kit::base::ThemeAppearance::Dark => WindowAppearance::Dark,
    }));
    theme.apply(cx);
    cx.global_mut::<State>().source = Source::Preview;
}

fn omarchy_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .is_some_and(|home| usable_omarchy_theme(std::path::Path::new(&home)))
    }
    #[cfg(not(target_os = "linux"))]
    false
}

#[cfg(target_os = "linux")]
fn usable_omarchy_theme(home: &std::path::Path) -> bool {
    let current = home.join(".local/state/omarchy/current");
    // Match gpui-omarchy's precedence, including broken current symlinks.
    let current = match std::fs::symlink_metadata(&current) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            home.join(".config/omarchy/current")
        }
        _ => current,
    };
    Theme::from_current_dir(current).is_ok()
}

fn native_palette(appearance: WindowAppearance) -> Theme {
    let fonts = Theme::tokyo_night();
    if matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    ) {
        Theme {
            name: "Dark".into(),
            appearance: gpui_kit::base::ThemeAppearance::Dark,
            background: rgb(0x18181b).into(),
            surface: rgb(0x27272a).into(),
            inset: rgb(0x09090b).into(),
            foreground: rgb(0xfafafa).into(),
            bright: rgb(0xfafafa).into(),
            secondary: rgb(0xa1a1aa).into(),
            accent: rgb(0x60a5fa).into(),
            on_accent: rgb(0x09090b).into(),
            selection: rgb(0x303e55).into(),
            border: rgb(0x3f3f46).into(),
            danger: rgb(0xf87171).into(),
            warning: rgb(0xfbbf24).into(),
            success: rgb(0x4ade80).into(),
            chart: [
                rgb(0x60a5fa).into(),
                rgb(0xc084fc).into(),
                rgb(0x2dd4bf).into(),
                rgb(0xfbbf24).into(),
                rgb(0x4ade80).into(),
            ],
            ..fonts
        }
    } else {
        Theme {
            name: "Light".into(),
            appearance: gpui_kit::base::ThemeAppearance::Light,
            background: rgb(0xfafafa).into(),
            surface: rgb(0xf4f4f5).into(),
            inset: rgb(0xe4e4e7).into(),
            foreground: rgb(0x18181b).into(),
            bright: rgb(0x18181b).into(),
            secondary: rgb(0x52525b).into(),
            accent: rgb(0x1d4ed8).into(),
            on_accent: rgb(0xffffff).into(),
            selection: rgb(0xdbeafe).into(),
            border: rgb(0xd4d4d8).into(),
            danger: rgb(0xb91c1c).into(),
            warning: rgb(0xb45309).into(),
            success: rgb(0x15803d).into(),
            chart: [
                rgb(0x1d4ed8).into(),
                rgb(0x7e22ce).into(),
                rgb(0x0f766e).into(),
                rgb(0xb45309).into(),
                rgb(0x15803d).into(),
            ],
            ..fonts
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod detection_tests {
    use super::usable_omarchy_theme;

    #[test]
    fn only_usable_current_themes_enable_omarchy_and_legacy_is_a_fallback() {
        let home = tempfile::tempdir().unwrap();
        assert!(!usable_omarchy_theme(home.path()));
        let legacy = home.path().join(".config/omarchy/current/theme");
        std::fs::create_dir_all(&legacy).unwrap();
        let palette = "background = '#18181b'\nforeground = '#fafafa'\naccent = '#60a5fa'\nred = '#f87171'\nyellow = '#fbbf24'\ngreen = '#4ade80'\n";
        std::fs::write(legacy.join("colors.toml"), palette).unwrap();
        assert!(usable_omarchy_theme(home.path()));
        let current = home.path().join(".local/state/omarchy/current");
        std::fs::create_dir_all(current.join("theme")).unwrap();
        std::fs::write(current.join("theme/colors.toml"), "invalid palette").unwrap();
        assert!(!usable_omarchy_theme(home.path()));
        std::fs::write(current.join("theme/colors.toml"), palette).unwrap();
        assert!(usable_omarchy_theme(home.path()));
        std::fs::remove_dir_all(&current).unwrap();
        std::os::unix::fs::symlink("missing", &current).unwrap();
        assert!(!usable_omarchy_theme(home.path()));
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use gpui_kit::TestAppContext;
    use gpui_omarchy::ActiveTheme;

    #[gpui_kit::test]
    fn native_appearance_updates_apply_only_in_system_mode(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_omarchy::init(cx);
            apply_with_omarchy(Appearance::System, false, cx);
            assert_eq!(cx.omarchy().name.as_ref(), "Light");
        });
        for (preference, os, expected) in [
            (Appearance::System, WindowAppearance::Dark, "Dark"),
            (Appearance::Light, WindowAppearance::VibrantDark, "Light"),
            (Appearance::Dark, WindowAppearance::Light, "Dark"),
            (Appearance::System, WindowAppearance::VibrantLight, "Light"),
            (Appearance::System, WindowAppearance::VibrantDark, "Dark"),
        ] {
            cx.update(|cx| {
                apply_with_omarchy(preference, false, cx);
                system_appearance_changed(os, cx);
            });
            cx.run_until_parked();
            cx.update(|cx| {
                assert_eq!(cx.omarchy().name.as_ref(), expected);
                let base = gpui_kit::base::Theme::global(cx);
                assert_eq!(base.appearance, cx.omarchy().appearance);
                assert_eq!(base.tokens.colors.background, cx.omarchy().background);
            });
        }
    }
}
