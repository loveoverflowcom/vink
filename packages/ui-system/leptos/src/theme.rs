//! Appearance preferences a caller injects into [`crate::UiRoot`] / [`crate::ThemeScope`].
//!
//! Precedence, nearest first: an explicit theme on the nearest scope, then enclosing scopes,
//! then `data-theme` on `<html>` (app-owned bootstrap), then the operating system. Density takes
//! the nearest declared value. Reduced motion is additive: the system setting, `<html>` or any
//! enclosing scope can reduce motion and nothing below can restore it.

use crate::tokens::ColorRole;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemePreference {
    Light,
    Dark,
    /// Follow `prefers-color-scheme`.
    #[default]
    System,
}

impl ThemePreference {
    pub const ALL: [ThemePreference; 3] = [Self::Light, Self::Dark, Self::System];

    /// The `data-theme` value and storage value apps already use.
    pub const fn value(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    /// Unknown or missing stored values fall back to the system.
    pub fn parse(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    /// The theme actually shown, given whether the system currently prefers dark.
    pub const fn resolve(self, system_dark: bool) -> ResolvedTheme {
        match self {
            Self::Light => ResolvedTheme::Light,
            Self::Dark => ResolvedTheme::Dark,
            Self::System if system_dark => ResolvedTheme::Dark,
            Self::System => ResolvedTheme::Light,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolvedTheme {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub const fn value(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MotionPreference {
    /// Follow `prefers-reduced-motion`.
    #[default]
    System,
    /// Reduce regardless of the system: transitions become instant and loops stop.
    Reduce,
}

/// What a root scope renders. Every field is chosen by the caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Preferences {
    pub theme: ThemePreference,
    pub density: Density,
    pub motion: MotionPreference,
}

/// Points one colour role at another inside a scope (`--vk-primary: var(--vk-tertiary)`).
/// Only roles can be targets — never a literal colour — so a screen cannot introduce a value
/// outside the token source, and the override keeps following the scope's theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoleOverride {
    pub role: ColorRole,
    pub to: ColorRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverrideError {
    /// Overriding a role with itself would make the custom property invalid.
    SelfReference(ColorRole),
    /// The same role is overridden twice in one scope.
    Duplicate(ColorRole),
    /// Overrides in one scope point at each other.
    Cycle(ColorRole),
}

impl RoleOverride {
    /// Inline `style` declarations for a scope, after rejecting overrides that would leave a
    /// custom property invalid at computed-value time.
    pub fn style(overrides: &[RoleOverride]) -> Result<String, OverrideError> {
        let mut seen = Vec::new();
        for item in overrides {
            if item.role == item.to {
                return Err(OverrideError::SelfReference(item.role));
            }
            if seen.contains(&item.role) {
                return Err(OverrideError::Duplicate(item.role));
            }
            seen.push(item.role);
        }
        // Follow overrides and recipe aliases declared on the same scope: reaching the role
        // again would leave both custom properties invalid at computed-value time.
        for item in overrides {
            let mut next = item.to;
            for _ in 0..=overrides.len() + ColorRole::ALL.len() {
                if next == item.role {
                    return Err(OverrideError::Cycle(item.role));
                }
                match overrides.iter().find(|o| o.role == next) {
                    Some(o) => next = o.to,
                    None => match next.alias() {
                        Some(target) => next = target,
                        None => break,
                    },
                }
            }
        }
        Ok(overrides
            .iter()
            .map(|o| format!("{}: var({});", o.role.css_var(), o.to.css_var()))
            .collect::<Vec<_>>()
            .join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_values_round_trip_and_unknown_falls_back_to_system() {
        for theme in ThemePreference::ALL {
            assert_eq!(ThemePreference::parse(theme.value()), theme);
        }
        assert_eq!(ThemePreference::parse("broken"), ThemePreference::System);
        assert_eq!(ThemePreference::parse(""), ThemePreference::System);
    }

    #[test]
    fn explicit_themes_win_over_the_system() {
        assert_eq!(ThemePreference::Light.resolve(true), ResolvedTheme::Light);
        assert_eq!(ThemePreference::Dark.resolve(false), ResolvedTheme::Dark);
        assert_eq!(ThemePreference::System.resolve(true), ResolvedTheme::Dark);
        assert_eq!(ThemePreference::System.resolve(false), ResolvedTheme::Light);
    }

    #[test]
    fn overrides_map_roles_to_roles_and_reject_invalid_sets() {
        let swap = RoleOverride {
            role: ColorRole::Primary,
            to: ColorRole::Tertiary,
        };
        assert_eq!(
            RoleOverride::style(&[swap]),
            Ok("--vk-primary: var(--vk-tertiary);".into())
        );
        let itself = RoleOverride {
            role: ColorRole::Primary,
            to: ColorRole::Primary,
        };
        assert_eq!(
            RoleOverride::style(&[itself]),
            Err(OverrideError::SelfReference(ColorRole::Primary))
        );
        assert_eq!(
            RoleOverride::style(&[swap, swap]),
            Err(OverrideError::Duplicate(ColorRole::Primary))
        );
        let back = RoleOverride {
            role: ColorRole::Tertiary,
            to: ColorRole::Primary,
        };
        assert_eq!(
            RoleOverride::style(&[swap, back]),
            Err(OverrideError::Cycle(ColorRole::Primary))
        );
        // `action.primary.container` is re-declared on every scope as var(--vk-primary).
        let through_recipe = RoleOverride {
            role: ColorRole::Primary,
            to: ColorRole::ActionPrimaryContainer,
        };
        assert_eq!(
            RoleOverride::style(&[through_recipe]),
            Err(OverrideError::Cycle(ColorRole::Primary))
        );
        let recipe = RoleOverride {
            role: ColorRole::ActionPrimaryContainer,
            to: ColorRole::Tertiary,
        };
        assert!(RoleOverride::style(&[recipe]).is_ok());
    }
}
