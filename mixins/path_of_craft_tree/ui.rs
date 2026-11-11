// The tree window around the tree: header, reset button, legend, vanilla-style tooltips, and
// the mouse (panning, zooming, learning a rank, switching a big node). paint.rs draws the tree.
use crate::game::{
    Game,
    native_hooks::NativeScreenView,
    pct_art as art,
    pct_glue::{ID, client},
    pct_look as look, pct_paint as paint, pct_rules as rules, pct_view as view,
};
use horizon_client::hud::HudBatch;
use look::{Look, Standing, State};
use paint::Scene;
use rules::{Kind, Node, Profile, Refusal, Tree};
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use view::Camera;

const LINE_HEIGHT: f32 = 10.0;
const TITLE_GAP: f32 = 2.0;
const WRAP_WIDTH: f32 = 210.0;
const WHITE: u32 = 0xFFFFFF;
const GREY: u32 = 0xAAAAAA;
const DARK: u32 = 0x777777;
const GREEN: u32 = 0x55FF55;
const RED: u32 = 0xFF5555;
const GOLD: u32 = 0xFFAA00;
const AQUA: u32 = 0x55FFFF;
const YELLOW: u32 = 0xFFFF55;
const TOOLTIP_FILL: u32 = 0xF0100010;
const BUTTON_SIZE: [f32; 2] = [130.0, 18.0];
const BUTTON_MARGIN: f32 = 6.0;
const HEADER_AT: [f32; 2] = [6.0, 7.0];
const LEFT_BUTTON: i32 = 0;
const RIGHT_BUTTON: i32 = 1;
const LEGEND: [(State, &str); 4] = [
    (State::Available, "можно изучить"),
    (State::Learned, "изучено"),
    (State::Locked, "закрыто"),
    (State::Off, "выключено ПКМ"),
];

static OPENED: LazyLock<Instant> = LazyLock::new(Instant::now);

fn is_ours(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(ID)
}

fn centre_of(screen: &NativeScreenView<'_>) -> [f32; 2] {
    let [x, y, width, height] = screen.rect;
    [x + width / 2.0, y + height / 2.0]
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] <= rect[0] + rect[2] && at[1] >= rect[1] && at[1] <= rect[1] + rect[3]
}

/// The node under `cursor`, front-most first.
fn hit_node<'a>(
    tree: &'a Tree,
    look: &Look,
    camera: Camera,
    centre: [f32; 2],
    cursor: [f32; 2],
) -> Option<&'a Node> {
    let level = look::level(look.spacing, camera.zoom);
    look.draw_order.iter().rev().find_map(|id| {
        let node = &tree.nodes[id];
        let [x, y] = camera.to_screen(centre, [node.x, node.y]);
        let half = look::hit_half(node.kind, level);
        ((cursor[0] - x).abs() <= half && (cursor[1] - y).abs() <= half).then_some(node)
    })
}

/// The node under the mouse, from a snapshot of the window's state.
fn node_under_mouse(screen: &NativeScreenView<'_>) -> Option<u16> {
    let (tree, camera) = {
        let view = client();
        (view.tree.clone()?, view.camera)
    };
    let look = art::look_for(&tree);
    hit_node(&tree, &look, camera, centre_of(screen), screen.mouse).map(|node| node.id)
}

fn respec_rect(screen: &NativeScreenView<'_>) -> [f32; 4] {
    let [x, y, width, _] = screen.rect;
    [
        x + width - BUTTON_SIZE[0] - BUTTON_MARGIN,
        y + BUTTON_MARGIN,
        BUTTON_SIZE[0],
        BUTTON_SIZE[1],
    ]
}

// Tooltips.

/// Splits `text` into lines no wider than the tooltip's width.
fn wrap(game: &Game<'_>, text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && game.native_text_width(&next) > WRAP_WIDTH {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn status_line(tree: &Tree, profile: &Profile, standing: &Standing, node: &Node) -> (String, u32) {
    let ranks = node.max_rank();
    let rank = profile.rank(node.id);
    let line = |text: &str, colour| (text.to_owned(), colour);
    match standing.state(node.id) {
        State::Off => return line("Выключено. ПКМ: включить", RED),
        State::Learned if node.kind == Kind::Big => return line("Изучено. ПКМ: выключить", GREY),
        State::Learned => return line("Изучено", GREY),
        _ => {}
    }
    match tree.can_allocate(profile, node.id) {
        Ok(()) if rank > 0 => (format!("Клик: ранг {}/{ranks}", rank + 1), GREEN),
        Ok(()) if ranks > 1 => (format!("Клик: изучить, ранг 1/{ranks}"), GREEN),
        Ok(()) => line("Клик: изучить", GREEN),
        Err(Refusal::NoPointsLeft) => line("Нет очков", RED),
        Err(_) => {
            let partial_neighbour = node
                .links
                .iter()
                .any(|link| profile.is_learned(*link) && !tree.is_full(profile, *link));
            if partial_neighbour {
                line("Сначала изучите соседний узел до конца", RED)
            } else {
                line("Нужен соседний узел", RED)
            }
        }
    }
}

fn tooltip_lines(
    game: &Game<'_>,
    tree: &Tree,
    look: &Look,
    profile: &Profile,
    standing: &Standing,
    node: &Node,
) -> Vec<(String, u32)> {
    let title = if node.kind == Kind::Big { GOLD } else { WHITE };
    let mut lines = vec![(node.name.clone(), title)];
    if let Some(branch) = node.branch.as_deref().and_then(|id| look.branch(id)) {
        lines.push((branch.name.clone(), look::mix(WHITE, branch.colour, 0.55)));
    }
    let ranks = node.max_rank();
    let repeated = ranks > 1 && node.rank_descriptions.contains(&node.description);
    if !node.description.is_empty() && !repeated {
        lines.extend(wrap(game, &node.description).into_iter().map(|t| (t, GREY)));
    }
    let rank = usize::from(profile.rank(node.id));
    if ranks > 1 {
        lines.push((format!("Ранг {rank}/{ranks}"), AQUA));
        for (i, text) in node.rank_descriptions.iter().enumerate() {
            let colour = match i.cmp(&rank) {
                std::cmp::Ordering::Less => WHITE,
                std::cmp::Ordering::Equal => GREEN,
                std::cmp::Ordering::Greater => DARK,
            };
            let wrapped = wrap(game, &format!("{}: {text}", i + 1));
            lines.extend(wrapped.into_iter().map(|t| (t, colour)));
        }
    }
    if node.kind == Kind::Small && !(ranks > 1 && !node.rank_descriptions.is_empty()) {
        lines.extend(
            node.effects
                .values()
                .map(|value| (format!("{:+.1}%", value * 100.0), GREEN)),
        );
    }
    if node.todo {
        lines.push(("Эффект ещё не реализован".to_owned(), RED));
    }
    lines.push(status_line(tree, profile, standing, node));
    lines
}

/// A box like a vanilla item tooltip: dark purple with a gradient border.
fn draw_tooltip(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    lines: &[(String, u32)],
) {
    let width = lines
        .iter()
        .map(|(text, _)| game.native_text_width(text))
        .fold(0.0, f32::max);
    let height = lines.len() as f32 * LINE_HEIGHT + TITLE_GAP - 2.0;
    let [rx, ry, rw, rh] = screen.rect;
    let x = (screen.mouse[0] + 12.0).min(rx + rw - width - 6.0);
    let y = (screen.mouse[1] - 12.0)
        .min(ry + rh - height - 6.0)
        .max(ry + 6.0);
    let fill = |batch: &mut HudBatch, rect, colour| game.native_fill(batch, rect, colour);
    fill(batch, [x - 3.0, y - 4.0, width + 6.0, 1.0], TOOLTIP_FILL);
    fill(
        batch,
        [x - 3.0, y + height + 3.0, width + 6.0, 1.0],
        TOOLTIP_FILL,
    );
    fill(
        batch,
        [x - 3.0, y - 3.0, width + 6.0, height + 6.0],
        TOOLTIP_FILL,
    );
    fill(batch, [x - 4.0, y - 3.0, 1.0, height + 6.0], TOOLTIP_FILL);
    fill(
        batch,
        [x + width + 3.0, y - 3.0, 1.0, height + 6.0],
        TOOLTIP_FILL,
    );
    let rows = (height + 6.0).max(1.0) as i32;
    for j in 0..rows {
        let t = j as f32 / (rows - 1).max(1) as f32;
        let channel = |top: f32, bottom: f32| (top + (bottom - top) * t).round() as u32;
        let colour = 0xFF00_0000
            | channel(80.0, 40.0) << 16
            | channel(20.0, 10.0) << 8
            | channel(190.0, 100.0);
        fill(batch, [x - 3.0, y - 3.0 + j as f32, 1.0, 1.0], colour);
        fill(
            batch,
            [x + width + 2.0, y - 3.0 + j as f32, 1.0, 1.0],
            colour,
        );
    }
    fill(batch, [x - 2.0, y - 3.0, width + 4.0, 1.0], 0xFF5014BE);
    fill(
        batch,
        [x - 2.0, y + height + 2.0, width + 4.0, 1.0],
        0xFF280A64,
    );
    let mut line_y = y;
    for (index, (text, colour)) in lines.iter().enumerate() {
        game.native_text(batch, text, [x, line_y], *colour, true);
        line_y += LINE_HEIGHT + if index == 0 { TITLE_GAP } else { 0.0 };
    }
}

// The header, the reset button and the legend.

/// Levels and points; the hovered node's branch first, its name in the branch colour.
fn draw_header(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    tree: &Tree,
    look: &Look,
    profile: &Profile,
    branch: Option<&str>,
) {
    let mut x = screen.rect[0] + HEADER_AT[0];
    let y = screen.rect[1] + HEADER_AT[1];
    let put = |batch: &mut HudBatch, x: &mut f32, text: &str, colour: u32| {
        game.native_text(batch, text, [*x, y], colour, true);
        *x += game.native_text_width(text);
    };
    let found = branch.and_then(|id| Some((id, look.branch(id)?)));
    if let Some((id, shown)) = found {
        let start = x;
        let xp = profile.xp.get(id).copied().unwrap_or(0.0);
        let (level, progress) = tree.level_progress(xp);
        put(
            batch,
            &mut x,
            &shown.name,
            look::mix(WHITE, shown.colour, 0.45),
        );
        let rest = format!(
            ": ур. {level} ({:.0}%), очки {}",
            progress * 100.0,
            tree.branch_points(profile, id)
        );
        put(batch, &mut x, &rest, WHITE);
        game.native_fill(
            batch,
            [start, y + LINE_HEIGHT, x - start, 2.0],
            0xFF00_0000 | shown.colour,
        );
        x += 14.0;
    }
    let general = format!(
        "Общие: ур. {}, очки {}",
        tree.general_level(profile),
        tree.general_points(profile)
    );
    put(batch, &mut x, &general, WHITE);
}

/// A stone button like the preview's: bevelled, red while it waits for a confirmation.
fn draw_respec(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>, armed: bool) {
    let [x, y, w, h] = respec_rect(screen);
    let hot = inside([x, y, w, h], screen.mouse);
    let (body, light, dark) = match (armed, hot) {
        (true, _) => (0xFF8A2A2A, 0xFFC04848, 0xFF5A1414),
        (false, true) => (0xFF7C7C86, 0xFF9A9AA4, 0xFF3C3C44),
        (false, false) => (0xFF5E5E66, 0xFF9A9AA4, 0xFF3C3C44),
    };
    game.native_fill(batch, [x, y, w, h], 0xFF08080A);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], body);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, 1.0], light);
    game.native_fill(batch, [x + 1.0, y + 1.0, 1.0, h - 3.0], light);
    game.native_fill(batch, [x + 1.0, y + h - 3.0, w - 2.0, 2.0], dark);
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
    let label = if armed {
        "Точно сбросить?".to_owned()
    } else {
        format!("Сброс: {} ур. опыта", rules::RESPEC_LEVELS)
    };
    let width = game.native_text_width(&label);
    let colour = if hot { YELLOW } else { WHITE };
    game.native_text(
        batch,
        &label,
        [(x + (w - width) / 2.0).round(), y + 5.0],
        colour,
        true,
    );
}

/// What the frame colours mean, in the bottom right corner.
fn draw_legend(scene: &Scene<'_, '_>, batch: &mut HudBatch) {
    let [rx, ry, rw, rh] = scene.rect;
    let mut x = rx + rw - 6.0;
    let y = ry + rh - 18.0;
    for (state, label) in LEGEND.iter().rev() {
        x -= scene.game.native_text_width(label);
        scene
            .game
            .native_text(batch, label, [x, y + 2.0], GREY, true);
        x -= 2.0 + paint::SWATCH;
        scene.swatch(batch, *state, [x, y]);
        x -= 8.0;
    }
}

pub fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::TreeDraw);
    if !is_ours(game, screen) {
        return;
    }
    let (tree, profile, problem, camera, armed) = {
        let view = client();
        (
            view.tree.clone(),
            view.profile.clone(),
            view.problem.clone(),
            view.camera,
            view.respec_armed,
        )
    };
    let [x, y, _, _] = screen.rect;
    match &tree {
        Some(tree) => draw_window(game, batch, screen, tree, &profile, camera, armed),
        None => {
            game.native_fill(batch, screen.rect, 0xD0101018);
            if problem.is_none() {
                game.native_text(
                    batch,
                    "Ожидание дерева от сервера…",
                    [x + 8.0, y + 8.0],
                    GREY,
                    true,
                );
            }
        }
    }
    if let Some(problem) = problem {
        game.native_text(batch, &problem, [x + 8.0, y + 22.0], RED, true);
    }
}

fn draw_window(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    tree: &Arc<Tree>,
    profile: &Profile,
    camera: Camera,
    armed: bool,
) {
    let look = art::look_for(tree);
    let standing = Standing::new(tree, profile);
    let centre = centre_of(screen);
    let scene = Scene {
        game,
        rect: screen.rect,
        centre,
        camera,
        tree,
        look: &look,
        standing: &standing,
        sheet: art::sheet(),
        level: look::level(look.spacing, camera.zoom),
        seconds: OPENED.elapsed().as_secs_f32(),
    };
    scene.draw(batch);
    let over_button = inside(respec_rect(screen), screen.mouse);
    let hovered = hit_node(tree, &look, camera, centre, screen.mouse).filter(|_| !over_button);
    if let Some(node) = hovered {
        scene.hover(batch, node);
    }
    let branch = hovered.and_then(|node| node.branch.as_deref());
    draw_header(game, batch, screen, tree, &look, profile, branch);
    draw_respec(game, batch, screen, armed);
    draw_legend(&scene, batch);
    if let Some(node) = hovered {
        let lines = tooltip_lines(game, tree, &look, profile, &standing, node);
        draw_tooltip(game, batch, screen, &lines);
    }
}

// The mouse.

pub fn on_press(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !is_ours(game, screen) {
        return;
    }
    if button == RIGHT_BUTTON {
        *canceled = true;
        if let Some(id) = node_under_mouse(screen) {
            client().toggles.push(id);
        }
        return;
    }
    if button != LEFT_BUTTON {
        return;
    }
    *canceled = true;
    let mut view = client();
    view.press = Some(screen.mouse);
    view.travelled = 0.0;
}

pub fn on_drag(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    delta: [f32; 2],
    canceled: &mut bool,
) {
    if button != LEFT_BUTTON || !is_ours(game, screen) {
        return;
    }
    let mut view = client();
    if view.press.is_some() {
        *canceled = true;
        view.camera = view.camera.panned(delta);
        view.travelled += delta[0].abs() + delta[1].abs();
    }
}

pub fn on_release(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if button != LEFT_BUTTON || !is_ours(game, screen) {
        return;
    }
    {
        let mut view = client();
        if view.press.take().is_none() {
            return;
        }
        *canceled = true;
        if view.travelled > view::CLICK_SLOP || view.problem.is_some() {
            return;
        }
        if inside(respec_rect(screen), screen.mouse) {
            view.respec_requested = view.respec_armed;
            view.respec_armed = !view.respec_armed;
            return;
        }
        view.respec_armed = false;
    }
    if let Some(id) = node_under_mouse(screen) {
        client().requests.push(id);
    }
}

pub fn on_scroll(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !is_ours(game, screen) {
        return;
    }
    *canceled = true;
    let mut view = client();
    view.camera = view
        .camera
        .zoomed_at(centre_of(screen), screen.mouse, scroll[1] as f32);
}
