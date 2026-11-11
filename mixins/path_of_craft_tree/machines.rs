// Reports from other mods' machines (RustCraft): the console command `pct-machine <report>`,
// which they run through `native_run_mod_command`. Players cannot send one, even operators:
// only the console path is heard. What a report means is in machine_rules.rs.
use crate::game::{
    Game, pct_actions as actions, pct_machine_rules as machine, pct_mining as mining,
};
use crate::mod_api::NativeCommand;
use machine::Report;

/// Registers the command, so the server hands it to the mods instead of vanilla's dispatcher.
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op(
        &format!("{} <report:text>", machine::COMMAND),
        4,
    ));
}

pub fn on_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let Some(line) = command
        .strip_prefix(machine::COMMAND)
        .and_then(|rest| rest.strip_prefix(' '))
    else {
        return;
    };
    *ran = true;
    match machine::parse(line) {
        Ok(report) => apply(game, report),
        Err(reason) => log::warn!("path_of_craft_tree: machine report refused ({reason}): {line}"),
    }
}

fn apply(game: &mut Game<'_>, report: Report) {
    match report {
        Report::Break {
            owner,
            dimension,
            pos,
            block,
            tool,
        } => mining::on_machine_break(game, owner, &dimension, pos, &block, tool),
        Report::Place {
            dimension,
            pos,
            block,
        } => mining::on_machine_place(game, &dimension, pos, &block),
        Report::Smelt { owner, item, count } => {
            actions::on_machine_smelt(game, owner, &item, count)
        }
        Report::Craft {
            owner,
            recipe,
            item,
            count,
            crafts,
        } => actions::on_machine_craft(game, owner, (&recipe, &item), count, crafts),
        Report::Kill {
            owner,
            entity,
            kind,
        } => actions::on_machine_kill(game, owner, entity, &kind),
    }
}
