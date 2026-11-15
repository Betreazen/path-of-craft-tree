// The optional experience bar on the screen: the level of the branch that earned last and the
// way to the next one, at the top in the middle, in the branch's colour. Off by default; the
// statistics page and a key in Options > Mods switch it. The label is laid out again only when
// a new profile arrives or the branch changes.
use crate::game::{
    Game, pct_glue::client, pct_look as look, pct_rules as rules, pct_window as window,
};
use horizon_client::hud::HudBatch;
use rules::{Profile, Tree};
use std::sync::{Arc, Mutex, PoisonError};

const WIDTH: f32 = 182.0;
const TOP: f32 = 3.0;
const BAR_Y: f32 = 13.0;
const BAR_H: f32 = 5.0;

struct Label {
    profile: Arc<Profile>,
    branch: String,
    text: String,
    text_width: f32,
    progress: f32,
    colour: u32,
}

static LABEL: Mutex<Option<Label>> = Mutex::new(None);

/// The branch to show: the one that earned last this session, else the one furthest along.
fn branch_for(tree: &Tree, profile: &Profile, last: Option<String>) -> Option<String> {
    last.filter(|b| tree.branches.contains_key(b)).or_else(|| {
        profile
            .xp
            .iter()
            .filter(|(b, _)| tree.branches.contains_key(*b))
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(b, _)| b.clone())
    })
}

fn label(game: &Game<'_>, tree: &Tree, profile: &Arc<Profile>, branch: String) -> Label {
    let xp = profile.xp.get(&branch).copied().unwrap_or(0.0);
    let (level, progress) = tree.level_progress(xp);
    let info = tree.branches.get(&branch);
    let name = info.map_or(branch.as_str(), |b| b.name.as_str());
    use crate::game::pct_texts::trf;
    let text = if level >= tree.curve.max_level {
        trf("{} · ур. {}", &[&name, &level])
    } else {
        let percent = format!("{:.0}", progress * 100.0);
        trf("{} · ур. {} · {}%", &[&name, &level, &percent])
    };
    Label {
        profile: profile.clone(),
        text_width: game.native_text_width(&text),
        text,
        progress: progress.clamp(0.0, 1.0) as f32,
        colour: look::parse_colour(info.and_then(|b| b.colour.as_deref())),
        branch,
    }
}

/// `native_hud`: the bar, when it is on and the tree is known.
pub fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    let Some(last) = window::bar_wanted() else {
        return;
    };
    let (tree, profile) = {
        let view = client();
        let Some(tree) = view.tree.clone() else {
            return;
        };
        (tree, view.profile.clone())
    };
    let Some(branch) = branch_for(&tree, &profile, last) else {
        return;
    };
    let mut cached = LABEL.lock().unwrap_or_else(PoisonError::into_inner);
    let fresh = cached
        .as_ref()
        .is_some_and(|l| Arc::ptr_eq(&l.profile, &profile) && l.branch == branch);
    if !fresh {
        *cached = Some(label(game, &tree, &profile, branch));
    }
    let Some(label) = cached.as_ref() else {
        return;
    };
    let x = ((batch.width - WIDTH) / 2.0).round();
    let text_x = ((batch.width - label.text_width) / 2.0).round();
    let name = look::mix(0xFFFFFF, label.colour, 0.45);
    game.native_text(batch, &label.text, [text_x, TOP], name, true);
    game.native_fill(batch, [x, BAR_Y, WIDTH, BAR_H], 0xFF000000);
    game.native_fill(
        batch,
        [x + 1.0, BAR_Y + 1.0, WIDTH - 2.0, BAR_H - 2.0],
        0xFF2A2A30,
    );
    let filled = ((WIDTH - 2.0) * label.progress).round();
    if filled > 0.0 {
        game.native_fill(
            batch,
            [x + 1.0, BAR_Y + 1.0, filled, BAR_H - 2.0],
            0xFF00_0000 | label.colour,
        );
    }
}
