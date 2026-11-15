// The server owner's settings: experience multipliers and group play, read from a JSON file in the
// server's config folder (settings.rs). Game-free, so `cargo test` covers every check. A setting
// that is missing or wrong keeps its default, so the defaults are exactly the mod's own rules.
use crate::game::pct_rules::XpScale;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// The file's name in the server's `config/` folder.
pub const FILE_NAME: &str = "path_of_craft_tree-server.json";

/// The highest multiplier a setting may give; 0 switches the experience off.
pub const MAX_MULTIPLIER: f64 = 10.0;
/// A group member near an earner gets this share of the earner's main experience, as extra.
pub const DEFAULT_GROUP_SHARE: f64 = 0.15;
pub const MAX_GROUP_SHARE: f64 = 0.5;
/// How near, in blocks, a member must be to an earner.
pub const DEFAULT_GROUP_RADIUS: f64 = 32.0;
pub const MIN_GROUP_RADIUS: f64 = 1.0;
pub const MAX_GROUP_RADIUS: f64 = 128.0;
/// Players in one group, the inviter included.
pub const DEFAULT_GROUP_SIZE: usize = 4;
pub const MIN_GROUP_SIZE: usize = 2;
pub const MAX_GROUP_SIZE: usize = 8;

/// Group play's numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub share: f64,
    pub radius: f64,
    pub max_size: usize,
}

impl Default for Group {
    fn default() -> Self {
        Group {
            share: DEFAULT_GROUP_SHARE,
            radius: DEFAULT_GROUP_RADIUS,
            max_size: DEFAULT_GROUP_SIZE,
        }
    }
}

/// Everything the file sets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub scale: XpScale,
    pub group: Group,
}

/// Reads the file. Never fails: what cannot be used is named in the warnings and keeps its
/// default, and a file that is not a JSON object gives the defaults whole.
pub fn parse(bytes: &[u8]) -> (Settings, Vec<String>) {
    let mut warnings = Vec::new();
    let mut settings = Settings::default();
    let root = match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(root)) => root,
        Ok(_) => {
            warnings.push("the file is not a JSON object: all settings are defaults".to_owned());
            return (settings, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "the file is not JSON ({error}): all settings are defaults"
            ));
            return (settings, warnings);
        }
    };
    for (key, value) in &root {
        match key.as_str() {
            "xp_multiplier" => {
                if let Some(n) = number(value, key, 0.0, MAX_MULTIPLIER, &mut warnings) {
                    settings.scale.global = n;
                }
            }
            "branch_multipliers" => {
                settings.scale.branches = branch_multipliers(value, &mut warnings);
            }
            "general_share" => {
                if !value.is_null() {
                    settings.scale.general_share = number(value, key, 0.0, 1.0, &mut warnings);
                }
            }
            "group" => settings.group = group(value, &mut warnings),
            // Notes for the owner, such as the template's `_about`.
            other if other.starts_with('_') => {}
            other => warnings.push(format!("unknown setting `{other}` ignored")),
        }
    }
    (settings, warnings)
}

/// A number of `min..=max`, else a warning naming `key` and None.
fn number(value: &Value, key: &str, min: f64, max: f64, warnings: &mut Vec<String>) -> Option<f64> {
    match value.as_f64() {
        Some(n) if n.is_finite() && (min..=max).contains(&n) => Some(n),
        _ => {
            warnings.push(format!(
                "`{key}` must be a number from {min} to {max}, not {value}: default kept"
            ));
            None
        }
    }
}

fn branch_multipliers(value: &Value, warnings: &mut Vec<String>) -> BTreeMap<String, f64> {
    let Some(map) = value.as_object() else {
        warnings.push(format!(
            "`branch_multipliers` must be an object of branch: number, not {value}: none used"
        ));
        return BTreeMap::new();
    };
    map.iter()
        .filter_map(|(branch, value)| {
            let key = format!("branch_multipliers.{branch}");
            number(value, &key, 0.0, MAX_MULTIPLIER, warnings).map(|n| (branch.clone(), n))
        })
        .collect()
}

fn group(value: &Value, warnings: &mut Vec<String>) -> Group {
    let mut group = Group::default();
    let Some(map) = value.as_object() else {
        warnings.push(format!(
            "`group` must be an object, not {value}: defaults kept"
        ));
        return group;
    };
    for (key, value) in map {
        let name = format!("group.{key}");
        match key.as_str() {
            "share" => {
                if let Some(n) = number(value, &name, 0.0, MAX_GROUP_SHARE, warnings) {
                    group.share = n;
                }
            }
            "radius" => {
                if let Some(n) = number(value, &name, MIN_GROUP_RADIUS, MAX_GROUP_RADIUS, warnings)
                {
                    group.radius = n;
                }
            }
            "max_size" => match value.as_u64().map(|n| n as usize) {
                Some(n) if (MIN_GROUP_SIZE..=MAX_GROUP_SIZE).contains(&n) => group.max_size = n,
                _ => warnings.push(format!(
                    "`{name}` must be a whole number from {MIN_GROUP_SIZE} to {MAX_GROUP_SIZE}, not {value}: default kept"
                )),
            },
            other if other.starts_with('_') => {}
            other => warnings.push(format!("unknown setting `group.{other}` ignored")),
        }
    }
    group
}

/// The settings with only the branches the tree has: the others are dropped with a warning.
pub fn known_branches(
    settings: &Settings,
    is_branch: impl Fn(&str) -> bool,
) -> (Settings, Vec<String>) {
    let mut kept = settings.clone();
    let mut warnings = Vec::new();
    kept.scale.branches.retain(|branch, _| {
        let known = is_branch(branch);
        if !known {
            warnings.push(format!(
                "`branch_multipliers.{branch}`: the tree has no such branch, ignored"
            ));
        }
        known
    });
    (kept, warnings)
}

/// One line for the log: what differs from the defaults.
pub fn describe(settings: &Settings) -> String {
    let scale = &settings.scale;
    let mut parts = vec![format!("xp ×{}", scale.global)];
    for (branch, n) in &scale.branches {
        parts.push(format!("{branch} ×{n}"));
    }
    if let Some(share) = scale.general_share {
        parts.push(format!("general share {share}"));
    }
    let group = &settings.group;
    parts.push(format!(
        "group share {}, radius {}, size {}",
        group.share, group.radius, group.max_size
    ));
    parts.join(", ")
}

/// The file written when there is none: every setting at its default.
pub fn template() -> String {
    let value = json!({
        "_about": "Path-of-craft-tree: настройки владельца сервера. Описание — в README мода, раздел «Настройки сервера». Перечитать без перезапуска: /pct admin reload.",
        "xp_multiplier": 1.0,
        "branch_multipliers": Map::new(),
        "general_share": null,
        "group": {
            "share": DEFAULT_GROUP_SHARE,
            "radius": DEFAULT_GROUP_RADIUS,
            "max_size": DEFAULT_GROUP_SIZE
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_reads_back_as_the_defaults_without_warnings() {
        let (settings, warnings) = parse(template().as_bytes());
        assert_eq!(settings, Settings::default());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(settings.scale.factor("mining"), 1.0);
    }

    #[test]
    fn an_empty_object_is_the_defaults() {
        let (settings, warnings) = parse(b"{}");
        assert_eq!(settings, Settings::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn every_setting_reads() {
        let json = br#"{"xp_multiplier": 2, "branch_multipliers": {"mining": 0.5, "fishing": 3},
            "general_share": 0.2, "group": {"share": 0.1, "radius": 16, "max_size": 3}}"#;
        let (settings, warnings) = parse(json);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(settings.scale.global, 2.0);
        assert_eq!(settings.scale.factor("mining"), 1.0);
        assert_eq!(settings.scale.factor("fishing"), 6.0);
        assert_eq!(settings.scale.factor("melee"), 2.0);
        assert_eq!(settings.scale.general_share, Some(0.2));
        assert_eq!(
            settings.group,
            Group {
                share: 0.1,
                radius: 16.0,
                max_size: 3
            }
        );
    }

    #[test]
    fn a_broken_file_gives_the_defaults_and_says_why() {
        for bytes in [&b"{ not json"[..], b"[1, 2]", b"", b"12"] {
            let (settings, warnings) = parse(bytes);
            assert_eq!(settings, Settings::default());
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(warnings[0].contains("defaults"), "{warnings:?}");
        }
    }

    #[test]
    fn a_wrong_value_keeps_its_default_and_the_rest_still_reads() {
        let json = br#"{"xp_multiplier": -1, "branch_multipliers": {"mining": "lots", "fishing": 2},
            "general_share": 1.5, "group": {"share": 0.9, "radius": 0, "max_size": 20}, "colour": 1}"#;
        let (settings, warnings) = parse(json);
        assert_eq!(settings.scale.global, 1.0);
        assert_eq!(
            settings.scale.branches,
            BTreeMap::from([("fishing".to_owned(), 2.0)])
        );
        assert_eq!(settings.scale.general_share, None);
        assert_eq!(settings.group, Group::default());
        assert_eq!(warnings.len(), 7, "{warnings:#?}");
        for name in [
            "xp_multiplier",
            "branch_multipliers.mining",
            "general_share",
            "group.share",
            "group.radius",
            "group.max_size",
            "colour",
        ] {
            assert!(
                warnings.iter().any(|w| w.contains(&format!("`{name}`"))),
                "{name}: {warnings:#?}"
            );
        }
    }

    #[test]
    fn wrong_shapes_are_refused_whole() {
        let (settings, warnings) = parse(br#"{"branch_multipliers": 2, "group": "big"}"#);
        assert_eq!(settings, Settings::default());
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn a_null_general_share_means_the_tree_s_own() {
        let (settings, warnings) = parse(br#"{"general_share": null}"#);
        assert_eq!(settings.scale.general_share, None);
        assert!(warnings.is_empty());
    }

    #[test]
    fn zero_switches_experience_off_and_ten_is_the_top() {
        let (settings, warnings) =
            parse(br#"{"xp_multiplier": 10, "branch_multipliers": {"mining": 0}}"#);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(settings.scale.factor("mining"), 0.0);
        assert_eq!(settings.scale.factor("melee"), 10.0);
        let (_, warnings) = parse(br#"{"xp_multiplier": 10.5}"#);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn branches_the_tree_lacks_are_dropped_with_a_warning() {
        let (settings, _) = parse(br#"{"branch_multipliers": {"mining": 2, "mineing": 3}}"#);
        let (kept, warnings) = known_branches(&settings, |b| b == "mining");
        assert_eq!(
            kept.scale.branches,
            BTreeMap::from([("mining".to_owned(), 2.0)])
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("mineing"));
    }

    #[test]
    fn notes_starting_with_an_underscore_are_allowed() {
        let (_, warnings) = parse(br#"{"_note": "x", "group": {"_why": "y"}}"#);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn the_description_names_what_is_set() {
        let (settings, _) = parse(
            br#"{"xp_multiplier": 2, "branch_multipliers": {"mining": 3}, "general_share": 0.1}"#,
        );
        let line = describe(&settings);
        for part in [
            "xp ×2",
            "mining ×3",
            "general share 0.1",
            "group share 0.15",
        ] {
            assert!(line.contains(part), "{line}");
        }
    }
}
