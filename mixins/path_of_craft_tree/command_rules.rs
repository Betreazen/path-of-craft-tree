// The `/pct` commands, free of game types: reading a command line, and what the admin commands do
// to a profile. commands.rs runs them.
use crate::game::pct_rules::{Profile, Tree};

/// The command's first word.
pub const ROOT: &str = "pct";
/// The operator level the admin commands need, as vanilla's `/experience`.
pub const ADMIN_LEVEL: u8 = 2;
/// What `give`, `take` and `level` call the general scale.
pub const GENERAL: &str = "general";
/// The most experience one admin command may give or take.
pub const MAX_ADMIN_XP: f64 = 1.0e9;

/// The usages the server registers: every player sees the first ones, operators the admin ones.
pub const PLAYER_USAGES: [&str; 6] = [
    "pct",
    "pct leaderboard [state:word]",
    "pct group invite <player:player_name>",
    "pct group accept",
    "pct group leave",
    "pct group list",
];
pub const ADMIN_USAGES: [&str; 6] = [
    "pct admin give <player:player_name> <branch:word> <amount:number>",
    "pct admin take <player:player_name> <branch:word> <amount:number>",
    "pct admin level <player:player_name> <branch:word> <level:integer>",
    "pct admin reset <player:player_name>",
    "pct admin show <player:player_name>",
    "pct admin reload",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    /// `None` asks how it stands.
    Leaderboard(Option<bool>),
    GroupInvite(String),
    GroupAccept,
    GroupLeave,
    GroupList,
    Admin(Admin),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Admin {
    /// Experience to add to a branch or the general scale; negative takes it away.
    Xp {
        player: String,
        branch: String,
        amount: f64,
    },
    Level {
        player: String,
        branch: String,
        level: u32,
    },
    Reset {
        player: String,
    },
    Show {
        player: String,
    },
    Reload,
}

impl Admin {
    /// Who the command is about, for the commands that name a player.
    pub fn player(&self) -> Option<&str> {
        match self {
            Admin::Xp { player, .. }
            | Admin::Level { player, .. }
            | Admin::Reset { player }
            | Admin::Show { player } => Some(player),
            Admin::Reload => None,
        }
    }
}

/// Reads a command line without its slash. None when it is not a `/pct` command; otherwise the
/// command or what is wrong with it.
pub fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    if words.next() != Some(ROOT) {
        return None;
    }
    let words: Vec<&str> = words.collect();
    Some(parse_words(&words))
}

fn parse_words(words: &[&str]) -> Result<Command, String> {
    match words {
        [] | ["help"] => Ok(Command::Help),
        ["leaderboard"] => Ok(Command::Leaderboard(None)),
        ["leaderboard", "on"] => Ok(Command::Leaderboard(Some(true))),
        ["leaderboard", "off"] => Ok(Command::Leaderboard(Some(false))),
        ["leaderboard", ..] => Err("/pct leaderboard on | off".to_owned()),
        ["group", "invite", name] => Ok(Command::GroupInvite((*name).to_owned())),
        ["group", "accept"] => Ok(Command::GroupAccept),
        ["group", "leave"] => Ok(Command::GroupLeave),
        ["group", "list"] => Ok(Command::GroupList),
        ["group", ..] => Err("/pct group invite <игрок> | accept | leave | list".to_owned()),
        ["admin", rest @ ..] => parse_admin(rest).map(Command::Admin),
        _ => Err("неизвестная команда, /pct help".to_owned()),
    }
}

fn parse_admin(words: &[&str]) -> Result<Admin, String> {
    let player = |name: &&str| (*name).to_owned();
    match words {
        [kind @ ("give" | "take"), name, branch, amount] => {
            let amount: f64 = amount
                .parse()
                .ok()
                .filter(|n: &f64| n.is_finite() && *n >= 0.0 && *n <= MAX_ADMIN_XP)
                .ok_or_else(|| format!("количество опыта от 0 до {MAX_ADMIN_XP}, не {amount}"))?;
            let amount = if *kind == "take" { -amount } else { amount };
            Ok(Admin::Xp {
                player: player(name),
                branch: (*branch).to_owned(),
                amount,
            })
        }
        ["level", name, branch, level] => Ok(Admin::Level {
            player: player(name),
            branch: (*branch).to_owned(),
            level: level
                .parse()
                .map_err(|_| format!("уровень — целое число от 0, не {level}"))?,
        }),
        ["reset", name] => Ok(Admin::Reset {
            player: player(name),
        }),
        ["show", name] => Ok(Admin::Show {
            player: player(name),
        }),
        ["reload"] => Ok(Admin::Reload),
        _ => Err(
            "/pct admin give|take <игрок> <ветвь|general> <опыт> | level <игрок> <ветвь|general> <уровень> | reset <игрок> | show <игрок> | reload"
                .to_owned(),
        ),
    }
}

/// A player written as a UUID, with or without dashes.
pub fn parse_uuid(word: &str) -> Option<u128> {
    let hex: String = word.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

fn check_branch(tree: &Tree, branch: &str) -> Result<(), String> {
    if branch == GENERAL || tree.branches.contains_key(branch) {
        return Ok(());
    }
    let known: Vec<&str> = tree.branches.keys().map(String::as_str).collect();
    Err(format!(
        "нет ветви {branch}; есть {GENERAL}, {}",
        known.join(", ")
    ))
}

/// The profile after an admin command that changes it. Experience never goes below 0, and
/// nodes the remaining points cannot pay for are given back (`Tree::sanitize`). Multipliers and
/// group shares do not apply: the admin sets exact numbers.
pub fn apply(tree: &Tree, profile: &Profile, admin: &Admin) -> Result<Profile, String> {
    let mut next = profile.clone();
    match admin {
        Admin::Xp { branch, amount, .. } => {
            check_branch(tree, branch)?;
            let xp = experience(&mut next, branch);
            *xp = (*xp + amount).max(0.0);
        }
        Admin::Level { branch, level, .. } => {
            check_branch(tree, branch)?;
            if *level > tree.curve.max_level {
                return Err(format!("уровень не выше {}", tree.curve.max_level));
            }
            *experience(&mut next, branch) = tree.xp_at_level(*level);
        }
        Admin::Reset { .. } => return Ok(tree.respec(profile)),
        Admin::Show { .. } | Admin::Reload => return Ok(next),
    }
    Ok(tree.sanitize(&next))
}

fn experience<'a>(profile: &'a mut Profile, branch: &str) -> &'a mut f64 {
    if branch == GENERAL {
        &mut profile.general_xp
    } else {
        profile.xp.entry(branch.to_owned()).or_insert(0.0)
    }
}

/// What `/pct admin show` prints about a profile: one line per scale, then the points.
pub fn describe(tree: &Tree, profile: &Profile) -> Vec<String> {
    let points = tree.points(profile);
    let mut lines: Vec<String> = tree
        .branches
        .iter()
        .map(|(id, branch)| {
            let xp = profile.xp.get(id).copied().unwrap_or(0.0);
            format!(
                "{} ({id}): уровень {}, опыт {xp:.1}, свободных очков {}",
                branch.name,
                tree.branch_level(profile, id),
                points.branch.get(id.as_str()).copied().unwrap_or(0)
            )
        })
        .collect();
    let ranks: u32 = profile.ranks.values().map(|rank| u32::from(*rank)).sum();
    lines.push(format!(
        "Общая шкала: уровень {}, опыт {:.1}, свободных очков {}",
        tree.general_level(profile),
        profile.general_xp,
        points.general
    ));
    lines.push(format!(
        "Изучено рангов: {}, выключено больших узлов: {}",
        ranks.saturating_sub(1),
        profile.disabled.len()
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(line: &str) -> Result<Command, String> {
        parse(line).expect("a /pct command")
    }

    #[test]
    fn other_commands_are_not_ours() {
        assert_eq!(parse("tree"), None);
        assert_eq!(parse("pct-machine smelt x"), None);
        assert_eq!(parse("pctx"), None);
    }

    #[test]
    fn player_commands_read() {
        assert_eq!(read("pct"), Ok(Command::Help));
        assert_eq!(read("pct leaderboard"), Ok(Command::Leaderboard(None)));
        assert_eq!(
            read("pct leaderboard on"),
            Ok(Command::Leaderboard(Some(true)))
        );
        assert_eq!(
            read("pct  leaderboard off"),
            Ok(Command::Leaderboard(Some(false)))
        );
        assert!(read("pct leaderboard maybe").is_err());
        assert_eq!(
            read("pct group invite Steve"),
            Ok(Command::GroupInvite("Steve".to_owned()))
        );
        assert_eq!(read("pct group accept"), Ok(Command::GroupAccept));
        assert_eq!(read("pct group leave"), Ok(Command::GroupLeave));
        assert_eq!(read("pct group list"), Ok(Command::GroupList));
        assert!(read("pct group invite").is_err());
        assert!(read("pct dance").is_err());
    }

    #[test]
    fn admin_commands_read() {
        assert_eq!(
            read("pct admin give Steve mining 120.5"),
            Ok(Command::Admin(Admin::Xp {
                player: "Steve".to_owned(),
                branch: "mining".to_owned(),
                amount: 120.5
            }))
        );
        assert_eq!(
            read("pct admin take Steve general 7"),
            Ok(Command::Admin(Admin::Xp {
                player: "Steve".to_owned(),
                branch: "general".to_owned(),
                amount: -7.0
            }))
        );
        assert_eq!(
            read("pct admin level Steve melee 30"),
            Ok(Command::Admin(Admin::Level {
                player: "Steve".to_owned(),
                branch: "melee".to_owned(),
                level: 30
            }))
        );
        assert_eq!(
            read("pct admin reset Steve"),
            Ok(Command::Admin(Admin::Reset {
                player: "Steve".to_owned()
            }))
        );
        assert_eq!(read("pct admin reload"), Ok(Command::Admin(Admin::Reload)));
        for broken in [
            "pct admin give Steve mining lots",
            "pct admin give Steve mining -5",
            "pct admin give Steve mining NaN",
            "pct admin give Steve mining 1e12",
            "pct admin level Steve melee -1",
            "pct admin reset",
            "pct admin",
        ] {
            assert!(read(broken).is_err(), "{broken}");
        }
    }

    #[test]
    fn every_usage_reads_as_a_command() {
        let fill = |usage: &str| {
            usage
                .split_whitespace()
                .map(|word| match word {
                    "<player:player_name>" => "Steve",
                    "<branch:word>" => "mining",
                    "<amount:number>" | "<level:integer>" => "3",
                    "[state:word]" => "on",
                    other => other,
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        for usage in PLAYER_USAGES.iter().chain(&ADMIN_USAGES) {
            assert!(read(&fill(usage)).is_ok(), "{usage}");
        }
    }

    #[test]
    fn uuids_read_with_or_without_dashes() {
        let id = 0x0123_abcd_4567_89ef_0123_4567_89ab_cdef_u128;
        assert_eq!(parse_uuid("0123abcd456789ef0123456789abcdef"), Some(id));
        assert_eq!(parse_uuid("0123abcd-4567-89ef-0123-456789abcdef"), Some(id));
        assert_eq!(parse_uuid("Steve"), None);
        assert_eq!(parse_uuid("0123abcd456789ef0123456789abcdeg"), None);
    }

    #[test]
    fn only_admin_commands_name_a_player() {
        assert_eq!(Admin::Reload.player(), None);
        assert_eq!(
            Admin::Show {
                player: "A".to_owned()
            }
            .player(),
            Some("A")
        );
    }
}
