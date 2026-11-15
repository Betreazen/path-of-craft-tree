// The `/pct` command (command_rules.rs): leaderboard consent and group play for every player,
// admin commands for operators of level 2 and the server console. Every admin change is logged.
use crate::game::{
    Game, pct_command_rules as rules, pct_glue as glue, pct_groups as groups,
    pct_leaderboard as leaderboard, pct_settings as settings,
};
use crate::mod_api::{NativeCommand, NativeServerPlayer};
use horizon_client::text::Text;
use rules::{Admin, Command};

/// Who typed the command.
pub enum Caller {
    Console,
    Player {
        uuid: u128,
        name: String,
        permission: u8,
    },
}

impl Caller {
    fn label(&self) -> String {
        match self {
            Caller::Console => "console".to_owned(),
            Caller::Player { uuid, name, .. } => format!("{name} ({uuid:032x})"),
        }
    }

    fn may_admin(&self) -> bool {
        match self {
            Caller::Console => true,
            Caller::Player { permission, .. } => *permission >= rules::ADMIN_LEVEL,
        }
    }
}

pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    for usage in rules::PLAYER_USAGES {
        commands.push(NativeCommand::new(usage));
    }
    for usage in rules::ADMIN_USAGES {
        commands.push(NativeCommand::op(usage, rules::ADMIN_LEVEL));
    }
}

/// A player's command on the server; their answer goes back as the command's feedback.
pub fn on_player_command(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let caller = Caller::Player {
        uuid: player.uuid,
        name: player.name.clone(),
        permission: player.permission,
    };
    answer(game, &caller, command, ran);
}

/// The same command typed on the server console: only the admin commands make sense there.
pub fn on_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    answer(game, &Caller::Console, command, ran);
}

fn answer(game: &mut Game<'_>, caller: &Caller, command: &str, ran: &mut bool) {
    let Some(lines) = run(game, caller, command) else {
        return;
    };
    *ran = true;
    // A player reads the answer in their own language; the console keeps Russian.
    let lang = match caller {
        Caller::Player { uuid, .. } => crate::game::pct_texts::player_lang(game, *uuid),
        Caller::Console => crate::game::pct_lang::Lang::Ru,
    };
    for line in lines {
        let line = crate::game::pct_texts::tr_in(lang, &line).into_owned();
        game.native_feedback(Text::literal(line));
    }
}

/// Runs `/pct …` for `caller`: the lines to answer with, or None when the command is not ours.
pub fn run(game: &mut Game<'_>, caller: &Caller, command: &str) -> Option<Vec<String>> {
    let parsed = rules::parse(command)?;
    Some(match parsed {
        Err(problem) => vec![problem],
        Ok(Command::Admin(admin)) if caller.may_admin() => run_admin(game, caller, &admin),
        Ok(Command::Admin(_)) => vec!["Эта команда только для операторов сервера".to_owned()],
        Ok(Command::Help) => help(caller),
        Ok(command) => match caller {
            Caller::Console => vec!["Эту команду вводит игрок, не консоль".to_owned()],
            Caller::Player { uuid, name, .. } => run_player(game, *uuid, name, command),
        },
    })
}

fn help(caller: &Caller) -> Vec<String> {
    let mut lines = vec![
        "/pct leaderboard on|off — показывать ли ваши уровни в таблице лидеров на сайте сервера".to_owned(),
        "/pct group invite <игрок> | accept | leave | list — группа: рядом с товарищем вы получаете долю его опыта".to_owned(),
    ];
    if caller.may_admin() {
        lines.push("/pct admin give|take <игрок> <ветвь|general> <опыт>, level <игрок> <ветвь|general> <уровень>, reset <игрок>, show <игрок>, reload".to_owned());
    }
    lines
}

fn run_player(game: &mut Game<'_>, uuid: u128, name: &str, command: Command) -> Vec<String> {
    match command {
        Command::Leaderboard(None) => vec![
            if leaderboard::is_on(game, uuid) {
                "Вы в таблице лидеров. Убрать: /pct leaderboard off"
            } else {
                "Вас нет в таблице лидеров. Показать: /pct leaderboard on"
            }
            .to_owned(),
        ],
        Command::Leaderboard(Some(on)) => match leaderboard::set(game, uuid, name, on) {
            Ok(_) if on => vec!["Ваши уровни теперь видны в таблице лидеров сайта".to_owned()],
            Ok(_) => vec!["Вас больше нет в таблице лидеров".to_owned()],
            Err(error) => vec![format!("Не получилось: {error}")],
        },
        Command::GroupInvite(target) => group_invite(game, uuid, name, &target),
        Command::GroupAccept => match groups::accept(uuid) {
            Ok(members) => {
                tell_group(&members, uuid, format!("{name} вступил в группу"));
                vec![format!("Вы в группе: {}", names(&members))]
            }
            Err(refusal) => vec![refusal.text().to_owned()],
        },
        Command::GroupLeave => match groups::leave(uuid) {
            Ok(left) => {
                tell_group(&left, uuid, format!("{name} покинул группу"));
                vec!["Вы покинули группу".to_owned()]
            }
            Err(refusal) => vec![refusal.text().to_owned()],
        },
        Command::GroupList => group_list(uuid),
        Command::Help | Command::Admin(_) => Vec::new(),
    }
}

fn group_invite(game: &Game<'_>, uuid: u128, name: &str, target: &str) -> Vec<String> {
    let Some((to, to_name)) = groups::find_online(game, target) else {
        return vec![format!("Игрок {target} не в сети")];
    };
    match groups::invite(uuid, to) {
        Ok(()) => {
            groups::tell_later(to, format!("{name} зовёт вас в группу: /pct group accept"));
            vec![format!("Приглашение отправлено: {to_name}")]
        }
        Err(refusal) => vec![refusal.text().to_owned()],
    }
}

fn group_list(uuid: u128) -> Vec<String> {
    let members = groups::members(uuid);
    let group = settings::group();
    let mut lines = if members.is_empty() {
        vec!["Вы не в группе".to_owned()]
    } else {
        vec![format!(
            "Группа ({} из {}): {}",
            members.len(),
            group.max_size,
            names(&members)
        )]
    };
    if let Some(from) = groups::invited_by(uuid) {
        lines.push(format!(
            "Вас зовёт {}: /pct group accept",
            groups::name_of(from)
        ));
    }
    lines.push(format!(
        "Доля опыта товарищам в {} блоках: {:.0}%",
        group.radius,
        group.share * 100.0
    ));
    lines
}

fn names(members: &[u128]) -> String {
    members
        .iter()
        .map(|m| groups::name_of(*m))
        .collect::<Vec<_>>()
        .join(", ")
}

fn tell_group(members: &[u128], except: u128, line: String) {
    for member in members.iter().filter(|m| **m != except) {
        groups::tell_later(*member, line.clone());
    }
}

/// A player named by an admin command: online by name, or anyone by UUID.
fn target(game: &Game<'_>, word: &str) -> Result<(u128, String), String> {
    if let Some(found) = groups::find_online(game, word) {
        return Ok(found);
    }
    match rules::parse_uuid(word) {
        Some(uuid) => Ok((uuid, format!("{uuid:032x}"))),
        None => Err(format!(
            "Игрок {word} не в сети; игрока не в сети укажите по UUID"
        )),
    }
}

fn run_admin(game: &mut Game<'_>, caller: &Caller, admin: &Admin) -> Vec<String> {
    if let Admin::Reload = admin {
        log::info!(
            "path_of_craft_tree: admin {}: reload the server settings",
            caller.label()
        );
        return settings::reload();
    }
    let Some(word) = admin.player() else {
        return Vec::new();
    };
    let (player, label) = match target(game, word) {
        Ok(found) => found,
        Err(problem) => return vec![problem],
    };
    if let Admin::Show { .. } = admin {
        return show(game, player, &label);
    }
    let Some(loaded) = glue::current_tree() else {
        return vec!["Дерево не загружено".to_owned()];
    };
    let tree = &loaded.tree;
    match glue::edit_profile(game, player, |tree, profile| {
        rules::apply(tree, profile, admin)
    }) {
        Ok((before, after)) => {
            let change = describe_change(tree, admin, &before, &after);
            log::info!(
                "path_of_craft_tree: admin {}: {label} ({player:032x}): {change}",
                caller.label()
            );
            if !glue::is_test_player(player) {
                groups::tell_later(
                    player,
                    format!("Администратор изменил ваш профиль: {change}"),
                );
            }
            vec![format!("{label}: {change}")]
        }
        Err(problem) => vec![format!("{label}: {problem}")],
    }
}

fn show(game: &Game<'_>, player: u128, label: &str) -> Vec<String> {
    let Some(loaded) = glue::current_tree() else {
        return vec!["Дерево не загружено".to_owned()];
    };
    match glue::admin_profile(game, player) {
        Ok((profile, online)) => {
            let status = if online {
                "в игре"
            } else {
                "не в сети"
            };
            let on_board = if leaderboard::is_on(game, player) {
                ", в таблице лидеров"
            } else {
                ""
            };
            let mut lines = vec![format!("{label} ({player:032x}), {status}{on_board}")];
            lines.extend(rules::describe(&loaded.tree, &profile));
            lines
        }
        Err(problem) => vec![format!("{label}: {problem}")],
    }
}

/// What an admin command changed, for its answer and the log.
fn describe_change(
    tree: &crate::game::pct_rules::Tree,
    admin: &Admin,
    before: &crate::game::pct_rules::Profile,
    after: &crate::game::pct_rules::Profile,
) -> String {
    let ranks = |p: &crate::game::pct_rules::Profile| -> u32 {
        p.ranks.values().map(|r| u32::from(*r)).sum::<u32>()
    };
    let refunded = ranks(before).saturating_sub(ranks(after));
    let refund = if refunded > 0 {
        format!(", возвращено рангов: {refunded}")
    } else {
        String::new()
    };
    let scale = |p: &crate::game::pct_rules::Profile, branch: &str| {
        if branch == rules::GENERAL {
            (p.general_xp, tree.general_level(p))
        } else {
            (
                p.xp.get(branch).copied().unwrap_or(0.0),
                tree.branch_level(p, branch),
            )
        }
    };
    match admin {
        Admin::Xp { branch, .. } | Admin::Level { branch, .. } => {
            let (xp_was, level_was) = scale(before, branch);
            let (xp_now, level_now) = scale(after, branch);
            format!(
                "{branch}: опыт {xp_was:.1} → {xp_now:.1}, уровень {level_was} → {level_now}{refund}"
            )
        }
        Admin::Reset { .. } => format!("дерево сброшено, возвращено рангов: {refunded}"),
        Admin::Show { .. } | Admin::Reload => String::new(),
    }
}
