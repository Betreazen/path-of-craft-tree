// Drawing the tree window's tools (window.rs) in the approved window's style: the search box and
// stone buttons under the header, the dimmed tree with the matches lit, the planner's path and
// its panel, and the statistics page. Panels are laid out once for what they show; a frame only
// copies their lines into the batch.
use crate::game::{
    Game, native_hooks::NativeScreenView, pct_look as look, pct_paint as paint, pct_plan as plan,
    pct_rules as rules, pct_session as session, pct_window as window,
};
use horizon_client::hud::HudBatch;
use look::{Look, Shown};
use paint::Scene;
use rules::{Profile, Tree};
use std::sync::Arc;
use window::{AQUA, Button, GREEN, GREY, Line, Panel, PlanView, RED, WHITE, Window};

/// The tool row under the header, and its parts.
const ROW_Y: f32 = 23.0;
const ROW_H: f32 = 14.0;
const SEARCH_W: f32 = 170.0;
const GAP: f32 = 4.0;
const PAD: f32 = 5.0;
const MARGIN: f32 = 6.0;
const LINE_H: f32 = 10.0;
const DARK: u32 = 0x777777;
const GOLD: u32 = 0xFFAA00;
/// Over the tree while a search is on, and the frames of the matches.
const DIM: u32 = 0xA8000000;
const MATCH: u32 = 0xFFFFE14D;
const CURRENT: u32 = 0xFFFFFFFF;
const PLAN_RING: u32 = 0xFF55FFFF;
const PLAN_LINK: [u32; 2] = [0xFF4FD8E8, 0xFF061418];
const PANEL_FILL: u32 = 0xE0101018;
const PANEL_EDGE: u32 = 0xFF3A3A48;
const STATS_W: f32 = 340.0;
const SOURCES_SHOWN: usize = 5;
/// The badge text of planned ranks.
const PLUS: [&str; 4] = ["+1", "+2", "+3", "+4"];

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] <= rect[0] + rect[2] && at[1] >= rect[1] && at[1] <= rect[1] + rect[3]
}

/// The button under the mouse, as drawn last frame.
pub fn button_at(_game: &Game<'_>, screen: &NativeScreenView<'_>) -> Option<Button> {
    let state = window::window();
    let stats = state.stats;
    state
        .hits
        .iter()
        .rev()
        .find(|(button, rect)| {
            inside(*rect, screen.mouse)
                && (!stats || matches!(button, Button::Bar | Button::Close | Button::Stats))
        })
        .map(|(button, _)| *button)
}

/// Whether the mouse is over a panel or the tool row, as drawn last frame.
pub fn covers(_game: &Game<'_>, screen: &NativeScreenView<'_>, state: &Window) -> bool {
    state.areas.iter().any(|rect| inside(*rect, screen.mouse))
        || state
            .hits
            .iter()
            .any(|(_, rect)| inside(*rect, screen.mouse))
}

// Over the tree.

/// The search's dimming and matches, and the planner's path, over the tree just drawn.
pub fn marks(
    scene: &Scene<'_, '_>,
    batch: &mut HudBatch,
    tree: &Arc<Tree>,
    profile: &Arc<Profile>,
) {
    let (found, view, current) = {
        let mut state = window::window();
        let found = state.found(tree, scene.look, profile, scene.shown);
        let view = state.planning.then(|| state.plan_view(tree, profile));
        (found, view, state.current)
    };
    if found.active {
        scene.game.native_fill(batch, scene.rect, DIM);
        let width = if scene.level >= 1.0 { 2.0 } else { 1.0 };
        for (index, id) in found.ids.iter().enumerate() {
            let Some(node) = tree.nodes.get(id) else {
                continue;
            };
            let colour = if current == Some(index) {
                CURRENT
            } else {
                MATCH
            };
            scene.ring(batch, node, colour, width);
            scene.node(batch, node);
        }
    }
    if let Some(view) = view {
        plan_marks(scene, batch, tree, &view);
    }
}

fn plan_marks(scene: &Scene<'_, '_>, batch: &mut HudBatch, tree: &Tree, view: &PlanView) {
    for (from, to) in &view.parents {
        if let (Some(a), Some(b)) = (tree.nodes.get(from), tree.nodes.get(to)) {
            scene.link_between(batch, a, b, PLAN_LINK);
        }
    }
    let width = if scene.level >= 1.0 { 2.0 } else { 1.0 };
    for id in &view.ids {
        let Some(node) = tree.nodes.get(id) else {
            continue;
        };
        scene.ring(batch, node, PLAN_RING, width);
        if scene.level < 1.0 {
            continue;
        }
        let ranks = usize::from(view.planned(*id)).clamp(1, PLUS.len());
        let at = scene.spot(scene.place(node));
        let half = look::hit_half(node.kind, scene.level);
        if scene.on_screen(at, half) {
            let spot = [at[0] - half, at[1] + half - 8.0];
            scene
                .game
                .native_text(batch, PLUS[ranks - 1], spot, AQUA & 0xFFFFFF, true);
        }
    }
}

// The tools.

fn text(game: &Game<'_>, batch: &mut HudBatch, words: &str, at: [f32; 2], colour: u32) {
    game.native_text(batch, words, at, colour, true);
}

/// A stone button like the reset button, green while its tool is on.
fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: [f32; 4],
    label: &str,
    on: bool,
    mouse: [f32; 2],
) {
    let [x, y, w, h] = rect;
    let hot = inside(rect, mouse);
    let (body, light, dark) = match (on, hot) {
        (true, false) => (0xFF2F6B2F, 0xFF4E9A4E, 0xFF173517),
        (true, true) => (0xFF3A803A, 0xFF5EB05E, 0xFF173517),
        (false, true) => (0xFF7C7C86, 0xFF9A9AA4, 0xFF3C3C44),
        (false, false) => (0xFF5E5E66, 0xFF9A9AA4, 0xFF3C3C44),
    };
    game.native_fill(batch, [x, y, w, h], 0xFF08080A);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], body);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, 1.0], light);
    game.native_fill(batch, [x + 1.0, y + 1.0, 1.0, h - 3.0], light);
    game.native_fill(batch, [x + 1.0, y + h - 2.0, w - 2.0, 1.0], dark);
    if hot {
        for edge in [
            [x, y, w, 1.0],
            [x, y + h - 1.0, w, 1.0],
            [x, y, 1.0, h],
            [x + w - 1.0, y, 1.0, h],
        ] {
            game.native_fill(batch, edge, 0xFFFFFFFF);
        }
    }
    let width = game.native_text_width(label);
    let colour = if hot { 0xFFFF55 } else { WHITE };
    let at = [
        (x + (w - width) / 2.0).round(),
        y + ((h - 8.0) / 2.0).round(),
    ];
    text(game, batch, label, at, colour);
}

/// Buttons in a row from `at`, each as wide as its label.
fn button_row(
    game: &Game<'_>,
    batch: &mut HudBatch,
    state: &mut Window,
    at: [f32; 2],
    buttons: &[(Button, &str, bool)],
    mouse: [f32; 2],
) -> f32 {
    let mut x = at[0];
    for (which, label, on) in buttons {
        let rect = [x, at[1], game.native_text_width(label) + 2.0 * PAD, ROW_H];
        button(game, batch, rect, label, *on, mouse);
        state.hits.push((*which, rect));
        x += rect[2] + GAP;
    }
    x
}

fn panel_box(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4]) {
    let [x, y, w, h] = rect;
    game.native_fill(batch, rect, PANEL_FILL);
    for edge in [
        [x, y, w, 1.0],
        [x, y + h - 1.0, w, 1.0],
        [x, y, 1.0, h],
        [x + w - 1.0, y, 1.0, h],
    ] {
        game.native_fill(batch, edge, PANEL_EDGE);
    }
}

fn panel_lines(game: &Game<'_>, batch: &mut HudBatch, panel: &Panel, at: [f32; 2]) {
    for (row, line) in panel.lines.iter().enumerate() {
        let y = at[1] + row as f32 * LINE_H;
        for (words, offset, colour) in &line.cells {
            text(game, batch, words, [at[0] + offset, y], *colour);
        }
    }
}

/// `lines` with the width of the widest one.
fn measured(game: &Game<'_>, lines: Vec<Line>) -> Panel {
    let width = lines
        .iter()
        .flat_map(|line| line.cells.iter())
        .map(|(words, offset, _)| offset + game.native_text_width(words))
        .fold(0.0, f32::max);
    Panel { lines, width }
}

/// What the search box shows of the query: its end when it is too long for the box.
fn shown_query(game: &Game<'_>, query: &str, room: f32) -> String {
    if plan::looks_like_share(query) && query.chars().count() > 16 {
        let tail: Vec<char> = query.chars().rev().take(10).collect();
        let tail: String = tail.into_iter().rev().collect();
        return format!("{}…{tail}", &query[..5.min(query.len())]);
    }
    let mut shown = query.to_owned();
    while !shown.is_empty() && game.native_text_width(&shown) > room {
        shown.remove(0);
    }
    shown
}

/// The tools for one frame: the tool row, the message, the planner and the statistics page.
#[allow(clippy::too_many_arguments)]
pub fn tools(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    tree: &Arc<Tree>,
    look: &Look,
    profile: &Arc<Profile>,
    shown: &Shown,
) {
    let mut state = window::window();
    state.hits.clear();
    state.areas.clear();
    let [rx, ry, rw, rh] = screen.rect;
    let mouse = if state.stats {
        [-1.0, -1.0]
    } else {
        screen.mouse
    };
    let row = [rx + MARGIN, ry + ROW_Y];
    search_box(game, batch, &mut state, tree, look, profile, shown, row);
    let (available, planning, stats) = (state.only_available, state.planning, state.stats);
    let buttons = [
        (Button::Available, "Можно взять", available),
        (Button::Planner, "Планировщик", planning),
        (Button::Stats, "Статистика", stats),
    ];
    let start = [row[0] + SEARCH_W + GAP, row[1]];
    button_row(game, batch, &mut state, start, &buttons, mouse);
    if let Some((words, colour)) = state.message() {
        let words = words.to_owned();
        text(game, batch, &words, [row[0], row[1] + ROW_H + 4.0], colour);
    }
    if state.planning {
        planner(
            game,
            batch,
            &mut state,
            tree,
            look,
            profile,
            [rx, ry, rw, rh],
            mouse,
        );
    }
    if state.stats {
        statistics(game, batch, &mut state, tree, look, profile, screen);
    }
}

#[allow(clippy::too_many_arguments)]
fn search_box(
    game: &Game<'_>,
    batch: &mut HudBatch,
    state: &mut Window,
    tree: &Arc<Tree>,
    look: &Look,
    profile: &Arc<Profile>,
    shown: &Shown,
    at: [f32; 2],
) {
    let rect = [at[0], at[1], SEARCH_W, ROW_H];
    let typing = !state.query.is_empty();
    let edge = if typing { 0xFFFFFFFF } else { 0xFFA0A0A0 };
    game.native_fill(batch, rect, edge);
    game.native_fill(
        batch,
        [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 2.0, rect[3] - 2.0],
        0xFF000000,
    );
    state.areas.push(rect);
    let found = state.found(tree, look, profile, shown);
    let (count, count_colour) = if plan::looks_like_share(&state.query) {
        ("Enter: загрузить".to_owned(), AQUA)
    } else if !found.active {
        (String::new(), GREY)
    } else if found.ids.is_empty() {
        ("нет".to_owned(), RED)
    } else {
        let at = state.current.map_or(0, |c| c + 1);
        (format!("{at}/{}", found.ids.len()), GREY)
    };
    let count_width = game.native_text_width(&count);
    let inner = [rect[0] + 4.0, rect[1] + 3.0];
    if count_width > 0.0 {
        let x = rect[0] + rect[2] - 4.0 - count_width;
        text(game, batch, &count, [x, inner[1]], count_colour);
    }
    if typing {
        let room = rect[2] - 14.0 - count_width;
        let words = shown_query(game, &state.query, room);
        text(game, batch, &words, inner, WHITE);
        let blink = state.started.elapsed().as_millis() / 500 % 2 == 0;
        if blink {
            let x = inner[0] + game.native_text_width(&words) + 1.0;
            text(game, batch, "_", [x, inner[1]], WHITE);
        }
    } else {
        text(game, batch, "Поиск: печатайте", inner, DARK);
    }
}

// The planner.

fn branch_name(look: &Look, id: &str) -> (String, u32) {
    match look.branch(id) {
        Some(branch) => (branch.name.clone(), look::mix(WHITE, branch.colour, 0.45)),
        None => ("Без ветви".to_owned(), GREY),
    }
}

/// The planner panel's lines for one plan view.
fn plan_panel(game: &Game<'_>, look: &Look, view: &PlanView) -> Panel {
    let cost = &view.cost;
    let mut lines = Vec::new();
    if view.steps == 0 {
        lines.push(Line::text(
            "План пуст: ЛКМ по узлу добавляет ранг, ПКМ убирает узел",
            GREY,
        ));
        lines.push(Line::text(
            "Ctrl+V или Enter в поиске: загрузить строку сборки",
            GREY,
        ));
        return measured(game, lines);
    }
    lines.push(Line::text(
        format!(
            "План: {} {}",
            cost.ranks,
            window::ranks_word(cost.ranks as usize)
        ),
        WHITE,
    ));
    let mut branches: Vec<_> = cost.branches.iter().collect();
    branches.sort_by(|a, b| b.1.planned.cmp(&a.1.planned).then(a.0.cmp(b.0)));
    const SHOWN: usize = 6;
    for (id, branch) in branches.iter().take(SHOWN) {
        let (name, colour) = branch_name(look, id);
        let mut cells = vec![(name, 0.0, colour)];
        let own = if id.is_empty() {
            "общими".to_owned()
        } else {
            format!("своих очков {}", branch.own)
        };
        cells.push((format!("{} ({own})", branch.planned), 110.0, WHITE));
        if branch.missing > 0 {
            cells.push((format!("не хватит {}", branch.missing), 210.0, RED));
        }
        lines.push(Line { cells });
    }
    if branches.len() > SHOWN {
        lines.push(Line::text(
            format!("…и ещё ветвей: {}", branches.len() - SHOWN),
            GREY,
        ));
    }
    lines.push(Line::text(
        format!(
            "Общие очки: нужно {}, есть {}",
            cost.general_used, cost.general_have
        ),
        GREY,
    ));
    if cost.missing > 0 {
        lines.push(Line::text(
            format!(
                "Не хватает очков: {}. Сейчас можно изучить {} из {}",
                cost.missing, cost.affordable, view.steps
            ),
            RED,
        ));
    } else {
        lines.push(Line::text("Очков хватает на весь план", GREEN));
    }
    measured(game, lines)
}

#[allow(clippy::too_many_arguments)]
fn planner(
    game: &Game<'_>,
    batch: &mut HudBatch,
    state: &mut Window,
    tree: &Arc<Tree>,
    look: &Look,
    profile: &Arc<Profile>,
    rect: [f32; 4],
    mouse: [f32; 2],
) {
    let view = state.plan_view(tree, profile);
    let panel = match &state.plan_lines {
        Some((known, panel)) if Arc::ptr_eq(known, &view) => panel.clone(),
        _ => {
            let panel = Arc::new(plan_panel(game, look, &view));
            state.plan_lines = Some((view.clone(), panel.clone()));
            panel
        }
    };
    let [rx, ry, _, rh] = rect;
    let buttons_y = ry + rh - MARGIN - ROW_H;
    let lines_h = panel.lines.len() as f32 * LINE_H;
    let top = buttons_y - GAP - lines_h - PAD;
    let buttons = [
        (Button::Apply, "Применить", false),
        (Button::Clear, "Очистить", false),
        (Button::Copy, "Копировать строку", false),
    ];
    let row_width: f32 = buttons
        .iter()
        .map(|(_, label, _)| game.native_text_width(label) + 2.0 * PAD + GAP)
        .sum();
    let width = panel.width.max(row_width) + 2.0 * PAD;
    let area = [
        rx + MARGIN,
        top - PAD,
        width,
        rh + ry - MARGIN - (top - PAD) + 2.0,
    ];
    panel_box(game, batch, area);
    state.areas.push(area);
    panel_lines(game, batch, &panel, [rx + MARGIN + PAD, top]);
    button_row(
        game,
        batch,
        state,
        [rx + MARGIN + PAD, buttons_y - 2.0],
        &buttons,
        mouse,
    );
}

// The statistics page.

fn duration_words(seconds: u64) -> String {
    let minutes = seconds / 60;
    if minutes < 60 {
        format!("{minutes} мин")
    } else {
        format!("{} ч {} мин", minutes / 60, minutes % 60)
    }
}

fn stats_panel(
    game: &Game<'_>,
    tree: &Tree,
    look: &Look,
    profile: &Profile,
    state: &Window,
    seconds: u64,
) -> Panel {
    let session = &state.session;
    let mut lines = vec![Line::text(
        format!("Статистика сессии · {}", duration_words(seconds)),
        GOLD,
    )];
    let branches = session.branches();
    if branches.is_empty() {
        lines.push(Line::text("За эту сессию опыта пока нет", GREY));
    } else {
        lines.push(Line {
            cells: vec![
                ("Ветвь".to_owned(), 0.0, DARK),
                ("Опыт".to_owned(), 110.0, DARK),
                ("Уровень".to_owned(), 170.0, DARK),
                ("В час".to_owned(), 250.0, DARK),
            ],
        });
    }
    for (id, xp) in &branches {
        let (name, colour) = branch_name(look, id);
        let (level, progress) = tree.level_progress(profile.xp.get(*id).copied().unwrap_or(0.0));
        let mut cells = vec![
            (name, 0.0, colour),
            (format!("+{xp:.0}"), 110.0, GREEN),
            (format!("{level} ({:.0}%)", progress * 100.0), 170.0, WHITE),
        ];
        if let Some(rate) = session::per_hour(*xp, seconds as f64) {
            cells.push((format!("{rate:.0}"), 250.0, GREY));
        }
        lines.push(Line { cells });
    }
    if session.general > 0.0 {
        lines.push(Line::text(
            format!("Общая шкала: +{:.0}", session.general),
            WHITE,
        ));
    }
    if session.knows_sources() {
        lines.push(Line::text("Лучшие источники", GOLD));
        for (branch, source, xp) in session.best_sources(SOURCES_SHOWN) {
            let (name, colour) = branch_name(look, branch);
            lines.push(Line {
                cells: vec![
                    (session::source_label(source), 0.0, WHITE),
                    (name, 170.0, colour),
                    (format!("+{xp:.0}"), 250.0, GREEN),
                ],
            });
        }
    } else if !branches.is_empty() {
        lines.push(Line::text("Источники опыта сервер пока не прислал", GREY));
    }
    measured(game, lines)
}

fn statistics(
    game: &Game<'_>,
    batch: &mut HudBatch,
    state: &mut Window,
    tree: &Arc<Tree>,
    look: &Look,
    profile: &Arc<Profile>,
    screen: &NativeScreenView<'_>,
) {
    let seconds = state.started.elapsed().as_secs();
    let key = (state.session.revision, seconds / 10, state.bar);
    let panel = match &state.stats_lines {
        Some((known, panel)) if *known == key => panel.clone(),
        _ => {
            let panel = Arc::new(stats_panel(game, tree, look, profile, state, seconds));
            state.stats_lines = Some((key, panel.clone()));
            panel
        }
    };
    let [rx, ry, rw, rh] = screen.rect;
    let width = panel.width.max(STATS_W) + 2.0 * PAD;
    let height = panel.lines.len() as f32 * LINE_H + ROW_H + 3.0 * PAD;
    let x = (rx + (rw - width) / 2.0).round();
    let y = (ry + ((rh - height) / 2.0).max(ROW_Y + ROW_H + 18.0)).round();
    game.native_fill(batch, screen.rect, 0x60000000);
    panel_box(game, batch, [x, y, width, height]);
    state.areas.push([x, y, width, height]);
    panel_lines(game, batch, &panel, [x + PAD, y + PAD]);
    let bar = if state.bar {
        "Полоса опыта на экране: вкл"
    } else {
        "Полоса опыта на экране: выкл"
    };
    let on = state.bar;
    let buttons = [(Button::Bar, bar, on), (Button::Close, "Закрыть", false)];
    let at = [x + PAD, y + height - PAD - ROW_H];
    button_row(game, batch, state, at, &buttons, screen.mouse);
}
