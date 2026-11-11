// Drawing the tree as the approved preview does (design/preview/render.js): the stone backdrop
// with its vignette and branch zones, links that turn gold when both ends are learned, framed
// pixel icons with rank badges, branch banners and names, and the energy running outward along
// learned links. Only what is on screen is drawn.
use crate::game::{Game, pct_art as art, pct_look as look, pct_rules as rules, pct_view as view};
use art::{Piece, Sheet};
use horizon_client::hud::HudBatch;
use look::{Look, Standing, State};
use rules::{Kind, Node, Tree};
use view::Camera;

const OUTLINE: u32 = 0xFF08080A;
const TEXT_OUTLINE: u32 = 0x0B0B0E;
const FLOW_GLOW: u32 = 0x61FFC440;
const FLOW_CORE: u32 = 0xFFFFF4B8;
const BACKDROP: u32 = 0xFF151519;
const HOVER: u32 = 0x40FFFFFF;
/// The backdrop's stone tile and the blocks its vignette darkens in.
const TILE: f32 = 128.0;
const BLOCK: f32 = 16.0;
const BAYER: [f32; 16] = [
    0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0,
];
const BANNER_TINT: f32 = 0.35;
/// The box a far-out branch name takes, outline included, and where the text sits inside it.
const NAME_HEIGHT: f32 = 10.0;
const NAME_INSET: f32 = 1.0;
/// The legend's frame side.
pub const SWATCH: f32 = 12.0;

/// A link's colours: its core and the outline around it.
const GOLD_LINK: [u32; 2] = [0xFFF2BB1D, 0xFF1A1003];
const OFF_LINK: [u32; 2] = [0xFF9A443E, 0xFF140707];
const OPEN_LINK: [u32; 2] = [0xFF4C9A3E, 0xFF08100A];
const PLAIN_LINK: [u32; 2] = [0xFF5C5C66, 0xFF0A0A0D];

/// Everything one frame of the window needs to draw the tree.
pub struct Scene<'a, 'g> {
    pub game: &'a Game<'g>,
    pub rect: [f32; 4],
    pub centre: [f32; 2],
    pub camera: Camera,
    pub tree: &'a Tree,
    pub look: &'a Look,
    pub standing: &'a Standing<'a>,
    pub sheet: Option<&'static Sheet>,
    pub level: f32,
    pub seconds: f32,
}

/// A frame's light and middle colour by state, for dots and plain frames.
pub fn state_colours(state: State) -> [u32; 2] {
    match state {
        State::Locked => [0xFF7A7A84, 0xFF50505A],
        State::Available | State::Partial => [0xFF8FE873, 0xFF3F9E37],
        State::Learned => [0xFFFFE680, 0xFFD39B14],
        State::Off => [0xFFDE877D, 0xFF96403A],
    }
}

impl Scene<'_, '_> {
    pub fn place(&self, node: &Node) -> [f32; 2] {
        self.camera.to_screen(self.centre, [node.x, node.y])
    }

    pub fn on_screen(&self, at: [f32; 2], margin: f32) -> bool {
        let [x, y, w, h] = self.rect;
        at[0] > x - margin && at[1] > y - margin && at[0] < x + w + margin && at[1] < y + h + margin
    }

    /// The size of one sprite pixel in GUI pixels: whole numbers keep the pixels square.
    fn unit(&self) -> f32 {
        if self.level >= 1.0 { self.level } else { 1.0 }
    }

    /// Where a node's sprite goes, snapped to the grid of its chunky pixels.
    pub fn spot(&self, at: [f32; 2]) -> [f32; 2] {
        let unit = self.unit();
        [(at[0] / unit).round() * unit, (at[1] / unit).round() * unit]
    }

    fn sprite(&self, batch: &mut HudBatch, piece: Piece, at: [f32; 2], scale: f32) {
        let [_, _, w, h] = piece.src;
        let dst = [
            at[0] - piece.anchor[0] * scale,
            at[1] - piece.anchor[1] * scale,
            w * scale,
            h * scale,
        ];
        self.game.native_sprite(batch, art::SPRITE, piece.src, dst);
    }

    /// Stone tiles with a dithered vignette, scrolling at half the speed of the tree.
    pub fn backdrop(&self, batch: &mut HudBatch) {
        let Some(sheet) = self.sheet else {
            self.game.native_fill(batch, self.rect, BACKDROP);
            return;
        };
        let [x0, y0, w, h] = self.rect;
        let zoom = self.camera.zoom;
        let shift = |pan: f32| (pan * zoom * 0.5).floor().rem_euclid(TILE);
        let (ox, oy) = (shift(self.camera.pan[0]), shift(self.camera.pan[1]));
        let (cols, rows) = ((w / BLOCK).ceil() as i32, (h / BLOCK).ceil() as i32);
        for row in 0..rows {
            for col in 0..cols {
                let (x, y) = (x0 + col as f32 * BLOCK, y0 + row as f32 * BLOCK);
                let r = ((x + BLOCK / 2.0 - x0 - w / 2.0) / (w / 2.0))
                    .hypot((y + BLOCK / 2.0 - y0 - h / 2.0) / (h / 2.0));
                let v = ((r - 0.45) / 0.85).clamp(0.0, 1.0);
                let dither = BAYER[((row & 3) * 4 + (col & 3)) as usize] / 16.0;
                let k = ((v * 5.0 + dither).floor() as usize).min(sheet.stone.len() - 1);
                let Some(stone) = sheet.stone[k] else {
                    continue;
                };
                self.stone_block(
                    batch,
                    stone,
                    [x, y],
                    [
                        (x - x0 - ox).rem_euclid(TILE),
                        (y - y0 - oy).rem_euclid(TILE),
                    ],
                );
            }
        }
    }

    /// One block of stone from tile position `uv`, split where it crosses the tile's edge.
    fn stone_block(&self, batch: &mut HudBatch, stone: Piece, at: [f32; 2], uv: [f32; 2]) {
        let first = [(TILE - uv[0]).min(BLOCK), (TILE - uv[1]).min(BLOCK)];
        for (dx, sx, wx) in [(0.0, uv[0], first[0]), (first[0], 0.0, BLOCK - first[0])] {
            for (dy, sy, hy) in [(0.0, uv[1], first[1]), (first[1], 0.0, BLOCK - first[1])] {
                if wx <= 0.0 || hy <= 0.0 {
                    continue;
                }
                let src = [stone.src[0] + sx, stone.src[1] + sy, wx, hy];
                let dst = [at[0] + dx, at[1] + dy, wx, hy];
                self.game.native_sprite(batch, art::SPRITE, src, dst);
            }
        }
    }

    /// The colour zones, stretched from their low-resolution map: only the visible cells.
    pub fn zones(&self, batch: &mut HudBatch) {
        let Some(origin) = art::zone_origin() else {
            return;
        };
        let z = &self.look.zones;
        let [x, y, w, h] = self.rect;
        let top_left = self.camera.to_world(self.centre, [x, y]);
        let bottom_right = self.camera.to_world(self.centre, [x + w, y + h]);
        let cell = |value: f32, start: f32, count: usize| {
            ((value - start) / z.cell).clamp(0.0, count as f32)
        };
        let (c0, c1) = (
            cell(top_left[0], z.x0, z.cols).floor(),
            cell(bottom_right[0], z.x0, z.cols).ceil(),
        );
        let (r0, r1) = (
            cell(top_left[1], z.y0, z.rows).floor(),
            cell(bottom_right[1], z.y0, z.rows).ceil(),
        );
        if c1 <= c0 || r1 <= r0 {
            return;
        }
        let corner = self
            .camera
            .to_screen(self.centre, [z.x0 + c0 * z.cell, z.y0 + r0 * z.cell]);
        let size = z.cell * self.camera.zoom;
        let src = [origin[0] + c0, origin[1] + r0, c1 - c0, r1 - r0];
        let dst = [corner[0], corner[1], (c1 - c0) * size, (r1 - r0) * size];
        self.game.native_sprite(batch, art::SPRITE, src, dst);
    }

    fn link_colours(&self, a: u16, b: u16) -> [u32; 2] {
        let s = self.standing;
        let (fa, fb) = (s.is_full(a), s.is_full(b));
        if fa && fb {
            if s.is_off(a) || s.is_off(b) {
                OFF_LINK
            } else {
                GOLD_LINK
            }
        } else if (fa && s.is_open(b)) || (fb && s.is_open(a)) {
            OPEN_LINK
        } else {
            PLAIN_LINK
        }
    }

    fn crosses_screen(&self, from: [f32; 2], to: [f32; 2]) -> bool {
        let [x, y, w, h] = self.rect;
        from[0].max(to[0]) >= x
            && from[0].min(to[0]) <= x + w
            && from[1].max(to[1]) >= y
            && from[1].min(to[1]) <= y + h
    }

    /// The part of a segment (0 to 1) inside the screen plus `pad`, as the preview clips it.
    fn clip(&self, from: [f32; 2], to: [f32; 2], pad: f32) -> Option<[f32; 2]> {
        let [x, y, w, h] = self.rect;
        let [dx, dy] = [to[0] - from[0], to[1] - from[1]];
        let edges = [
            (-dx, from[0] - x + pad),
            (dx, x + w + pad - from[0]),
            (-dy, from[1] - y + pad),
            (dy, y + h + pad - from[1]),
        ];
        let mut range = [0.0f32, 1.0];
        for (p, q) in edges {
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
            } else if p < 0.0 {
                range[0] = range[0].max(q / p);
            } else {
                range[1] = range[1].min(q / p);
            }
        }
        (range[0] <= range[1]).then_some(range)
    }

    /// A pixel path walked in blocks of `unit` GUI pixels, `width` blocks thick; a run of blocks
    /// on one row (or column) is one rectangle.
    fn stepped(
        &self,
        batch: &mut HudBatch,
        from: [f32; 2],
        to: [f32; 2],
        unit: f32,
        width: f32,
        colour: u32,
    ) {
        let Some(range) = self.clip(from, to, width * unit) else {
            return;
        };
        let [ax, ay] = [(from[0] / unit).round(), (from[1] / unit).round()];
        let [dx, dy] = [(to[0] / unit).round() - ax, (to[1] / unit).round() - ay];
        let steps = dx.abs().max(dy.abs()).max(1.0);
        let across = ((width - 1.0) / 2.0).floor();
        let flat = dx.abs() >= dy.abs();
        let cell = |i: f32| [ax + (dx * i / steps).round(), ay + (dy * i / steps).round()];
        let emit = |batch: &mut HudBatch, first: [f32; 2], last: [f32; 2]| {
            let rect = if flat {
                let x0 = first[0].min(last[0]);
                [
                    x0 * unit,
                    (first[1] - across) * unit,
                    ((first[0] - last[0]).abs() + 1.0) * unit,
                    width * unit,
                ]
            } else {
                let y0 = first[1].min(last[1]);
                [
                    (first[0] - across) * unit,
                    y0 * unit,
                    width * unit,
                    ((first[1] - last[1]).abs() + 1.0) * unit,
                ]
            };
            self.game.native_fill(batch, rect, colour);
        };
        let (begin, end) = ((range[0] * steps).floor(), (range[1] * steps).ceil());
        let mut run = cell(begin);
        let mut previous = run;
        let mut i = begin + 1.0;
        while i <= end {
            let here = cell(i);
            let same = if flat {
                here[1] == run[1]
            } else {
                here[0] == run[0]
            };
            if !same {
                emit(batch, run, previous);
                run = here;
            }
            previous = here;
            i += 1.0;
        }
        emit(batch, run, previous);
    }

    /// Every visible link as a chunky pixel line: outlines first, then the coloured cores.
    pub fn links(&self, batch: &mut HudBatch) {
        let unit = self.unit();
        let (core, outline) = if self.level >= 1.0 {
            (2.0, 4.0)
        } else if self.level > 0.0 {
            (1.0, 3.0)
        } else {
            (1.0, 0.0)
        };
        let visible: Vec<([f32; 2], [f32; 2], [u32; 2])> = self
            .look
            .edges
            .iter()
            .filter_map(|(a, b)| {
                let (from, to) = (
                    self.place(&self.tree.nodes[a]),
                    self.place(&self.tree.nodes[b]),
                );
                self.crosses_screen(from, to)
                    .then(|| (from, to, self.link_colours(*a, *b)))
            })
            .collect();
        if outline > 0.0 {
            for (from, to, colours) in &visible {
                self.stepped(batch, *from, *to, unit, outline, colours[1]);
            }
        }
        for (from, to, colours) in &visible {
            self.stepped(batch, *from, *to, unit, core, colours[0]);
        }
    }

    /// Zoomed far out: a coloured dot per node.
    fn dot(&self, batch: &mut HudBatch, node: &Node, at: [f32; 2], state: State) {
        let roomy = self.look.spacing * self.camera.zoom > 7.0;
        let size = match (node.kind, roomy) {
            (Kind::Start, _) => 7.0,
            (Kind::Big, true) => 5.0,
            (Kind::Big, false) => 4.0,
            (_, true) => 3.0,
            (_, false) => 2.0,
        };
        let [x, y] = [(at[0] - size / 2.0).round(), (at[1] - size / 2.0).round()];
        if matches!(node.kind, Kind::Big | Kind::Start) {
            self.game
                .native_fill(batch, [x - 1.0, y - 1.0, size + 2.0, size + 2.0], OUTLINE);
        }
        let [light, middle] = state_colours(state);
        let colour = if state == State::Locked {
            middle
        } else {
            light
        };
        self.game.native_fill(batch, [x, y, size, size], colour);
    }

    /// A node's frame, icon and rank badge, or a plain square when the art is missing.
    fn framed(&self, batch: &mut HudBatch, node: &Node, at: [f32; 2], state: State) {
        let compact = self.level < 1.0;
        let scale = self.unit();
        let spot = self.spot(at);
        let side = look::frame_side(node.kind, compact);
        let Some(sheet) = self.sheet else {
            let half = side / 2.0 * scale;
            let box_ = [spot[0] - half, spot[1] - half, half * 2.0, half * 2.0];
            self.game.native_fill(batch, box_, state_colours(state)[1]);
            return;
        };
        if let Some(frame) = sheet.frame(node.kind, state, compact) {
            self.sprite(batch, frame, spot, scale);
        }
        if let Some(icon) = sheet.icon(&node.icon, state, compact) {
            self.sprite(batch, icon, spot, scale);
        }
        let ranks = node.max_rank();
        if ranks < 2 {
            return;
        }
        let Some(badge) = sheet.badge(self.standing.rank(node.id), ranks, compact) else {
            return;
        };
        let reach = if compact {
            2.0
        } else if node.kind == Kind::Big {
            1.0
        } else {
            3.0
        };
        let right = spot[0] + (side / 2.0 + reach) * scale;
        let top = spot[1] - (side / 2.0 + reach) * scale;
        let [_, _, w, h] = badge.src;
        let dst = [right - w * scale, top, w * scale, h * scale];
        self.game.native_sprite(batch, art::SPRITE, badge.src, dst);
    }

    pub fn nodes(&self, batch: &mut HudBatch) {
        let margin = 40.0 * self.level.max(1.0);
        for id in &self.look.draw_order {
            let node = &self.tree.nodes[id];
            let at = self.place(node);
            if !self.on_screen(at, margin) {
                continue;
            }
            let state = self.standing.state(*id);
            if self.level > 0.0 {
                self.framed(batch, node, at, state);
            } else {
                self.dot(batch, node, at, state);
            }
        }
    }

    /// A small empty frame in `state`'s colours, its top left at `at`, for the legend.
    pub fn swatch(&self, batch: &mut HudBatch, state: State, at: [f32; 2]) {
        let centre = [at[0] + SWATCH / 2.0, at[1] + SWATCH / 2.0];
        let pieces = self.sheet.and_then(|sheet| {
            Some((
                sheet.frame(Kind::Small, state, true)?,
                sheet.icon("?", state, true)?,
            ))
        });
        match pieces {
            Some((frame, icon)) => {
                self.sprite(batch, frame, centre, 1.0);
                self.sprite(batch, icon, centre, 1.0);
            }
            None => self.game.native_fill(
                batch,
                [at[0], at[1], SWATCH, SWATCH],
                state_colours(state)[0],
            ),
        }
    }

    /// A light wash over the node under the mouse.
    pub fn hover(&self, batch: &mut HudBatch, node: &Node) {
        if self.level <= 0.0 {
            return;
        }
        let spot = self.spot(self.place(node));
        let half = look::hit_half(node.kind, self.level);
        self.game.native_fill(
            batch,
            [spot[0] - half, spot[1] - half, half * 2.0, half * 2.0],
            HOVER,
        );
    }

    /// Text with a dark outline all round, as the preview's branch names have.
    pub fn outlined(&self, batch: &mut HudBatch, text: &str, at: [f32; 2], colour: u32) {
        for [dx, dy] in [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0], [1.0, 1.0]] {
            self.game
                .native_text(batch, text, [at[0] + dx, at[1] + dy], TEXT_OUTLINE, false);
        }
        self.game.native_text(batch, text, at, colour, false);
    }

    /// Banners where the branches leave the centre; zoomed far out, just the names.
    pub fn branches(&self, batch: &mut HudBatch) {
        if self.level > 0.0 {
            self.banners(batch);
        } else {
            self.names(batch);
        }
    }

    fn banners(&self, batch: &mut HudBatch) {
        let scale = self.level.clamp(1.0, 2.0);
        for branch in &self.look.branches {
            let at = self.camera.to_screen(self.centre, branch.anchor);
            if !self.on_screen(at, 120.0) {
                continue;
            }
            let width = self.game.native_text_width(&branch.name);
            let banner = self.sheet.and_then(|sheet| sheet.banner(&branch.id));
            let (x, y) = match banner {
                Some(banner) => {
                    let spot = [
                        (at[0] / scale).round() * scale,
                        (at[1] / scale).round() * scale,
                    ];
                    self.sprite(batch, banner, spot, scale);
                    let below = spot[1] + (banner.src[3] - banner.anchor[1]) * scale + 2.0;
                    (spot[0] - width / 2.0, below)
                }
                None => (at[0] - width / 2.0, at[1] - 4.0),
            };
            let colour = look::mix(0xFFFFFF, branch.colour, BANNER_TINT);
            self.outlined(batch, &branch.name, [x.round(), y.round()], colour);
        }
    }

    /// Zoomed far out, just the names, moved apart on screen where the game font makes them meet.
    fn names(&self, batch: &mut HudBatch) {
        let origin = self
            .tree
            .nodes
            .get(&self.tree.start)
            .map_or(self.centre, |start| {
                self.camera.to_screen(self.centre, [start.x, start.y])
            });
        let wanted: Vec<([f32; 2], f32)> = self
            .look
            .branches
            .iter()
            .map(|branch| {
                let at = self.camera.to_screen(self.centre, branch.label);
                (at, self.game.native_text_width(&branch.name))
            })
            .collect();
        let boxes = look::spread_labels(origin, &wanted, NAME_HEIGHT);
        for (branch, spot) in self.look.branches.iter().zip(boxes) {
            if !self.on_screen([spot[0], spot[1]], 120.0) {
                continue;
            }
            let colour = look::mix(0xFFFFFF, branch.colour, BANNER_TINT);
            let at = [spot[0].round(), (spot[1] + NAME_INSET).round()];
            self.outlined(batch, &branch.name, at, colour);
        }
    }

    /// One small glowing square per learned link, travelling outward from the start.
    pub fn flow(&self, batch: &mut HudBatch) {
        let core = if self.level >= 1.0 {
            2.0 * self.level
        } else {
            2.0
        };
        let snap = |value: f32| ((value - core / 2.0) / (core / 2.0)).round() * (core / 2.0);
        let s = self.standing;
        let mut spots = Vec::new();
        for (a, b) in &self.look.edges {
            if !s.is_full(*a) || !s.is_full(*b) || s.is_off(*a) || s.is_off(*b) {
                continue;
            }
            let (da, db) = (self.look.dist[a], self.look.dist[b]);
            let (from, to) = if da <= db { (a, b) } else { (b, a) };
            let (p, q) = (&self.tree.nodes[from], &self.tree.nodes[to]);
            let length = (q.x - p.x).hypot(q.y - p.y);
            let Some(t) = look::flow_at(self.seconds, da.min(db), length) else {
                continue;
            };
            let (sp, sq) = (self.place(p), self.place(q));
            let at = [
                snap(sp[0] + (sq[0] - sp[0]) * t),
                snap(sp[1] + (sq[1] - sp[1]) * t),
            ];
            if self.on_screen(at, core) {
                spots.push(at);
            }
        }
        for at in &spots {
            let glow = [at[0] - 1.0, at[1] - 1.0, core + 2.0, core + 2.0];
            self.game.native_fill(batch, glow, FLOW_GLOW);
        }
        for at in &spots {
            self.game
                .native_fill(batch, [at[0], at[1], core, core], FLOW_CORE);
        }
    }

    /// The tree in the preview's layers.
    pub fn draw(&self, batch: &mut HudBatch) {
        self.backdrop(batch);
        self.zones(batch);
        self.links(batch);
        self.nodes(batch);
        self.branches(batch);
        self.flow(batch);
    }
}
