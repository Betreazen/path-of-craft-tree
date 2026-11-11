// The tree window's pixel art: our own picture (data/atlas.png, packed by
// design/tools/game-atlas.mjs) in the game's mod image slot, with the colour zones of the tree
// the server sent laid into its free rows, and the table that finds each piece in it.
use crate::game::{Game, pct_glue as glue, pct_look as look, pct_rules as rules};
use look::{Look, State};
use rules::{Kind, Tree};
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

const ATLAS_PNG: &[u8] = horizon_client::native_data!("path_of_craft_tree", "atlas.png");
const ATLAS_JSON: &[u8] = horizon_client::native_data!("path_of_craft_tree", "atlas.json");
/// The engine's slot for a mod's own image (`crate::hud::MOD_IMAGE_SPRITE`).
pub const SPRITE: &str = "horizon/modimage";
const KINDS: [Kind; 4] = [Kind::Small, Kind::Upgrade, Kind::Big, Kind::Start];
/// Icon variants: as drawn, greyed for locked nodes, reddened for switched-off ones.
const VARIANTS: [&str; 3] = ["n", "l", "o"];
const STONE_LEVELS: usize = 6;

/// One sprite: where it is in the picture, and its anchor (a node's centre) inside it.
#[derive(Debug, Clone, Copy)]
pub struct Piece {
    pub src: [f32; 4],
    pub anchor: [f32; 2],
}

pub struct Sheet {
    frames: [[[Option<Piece>; 5]; 4]; 2],
    icons: HashMap<String, [[Option<Piece>; 3]; 2]>,
    badges: HashMap<(u8, u8), [Option<Piece>; 2]>,
    banners: HashMap<String, Piece>,
    pub stone: [Option<Piece>; STONE_LEVELS],
    pixels: Vec<u8>,
    width: u32,
    rows: u32,
    free: [u32; 4],
}

fn decode() -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(ATLAS_PNG));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("atlas too large")?];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "atlas is {:?} {:?}, not RGBA",
            info.color_type, info.bit_depth
        ));
    }
    pixels.truncate(info.buffer_size());
    Ok((pixels, info.width, info.height))
}

fn piece(table: &serde_json::Map<String, serde_json::Value>, name: &str) -> Option<Piece> {
    let values: Vec<f32> = table
        .get(name)?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_f64().map(|n| n as f32))
        .collect();
    let [x, y, w, h, cx, cy] = <[f32; 6]>::try_from(values).ok()?;
    Some(Piece {
        src: [x, y, w, h],
        anchor: [cx, cy],
    })
}

fn load() -> Result<Sheet, String> {
    let (pixels, width, rows) = decode()?;
    let json: serde_json::Value = serde_json::from_slice(ATLAS_JSON).map_err(|e| e.to_string())?;
    let table = json["sprites"]
        .as_object()
        .ok_or("atlas.json has no sprites")?;
    let free: Vec<u32> = json["free"]
        .as_array()
        .ok_or("atlas.json has no free area")?
        .iter()
        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
        .collect();
    let free = <[u32; 4]>::try_from(free).map_err(|_| "atlas.json's free area is not 4 numbers")?;
    let mut frames = [[[None; 5]; 4]; 2];
    for (c, prefix) in ["frame", "cframe"].iter().enumerate() {
        for (k, kind) in KINDS.iter().enumerate() {
            for state in State::ALL {
                let name = format!("{prefix}:{}:{}", look::kind_name(*kind), state.name());
                frames[c][k][state.index()] = piece(table, &name);
            }
        }
    }
    let mut icons = HashMap::new();
    let mut badges = HashMap::new();
    let mut banners = HashMap::new();
    for name in table.keys() {
        if let Some(rest) = name.strip_prefix("icon:") {
            let Some((id, _)) = rest.rsplit_once(':') else {
                continue;
            };
            let entry = icons.entry(id.to_owned()).or_insert([[None; 3]; 2]);
            for (v, variant) in VARIANTS.iter().enumerate() {
                entry[0][v] = piece(table, &format!("icon:{id}:{variant}"));
                entry[1][v] = piece(table, &format!("cicon:{id}:{variant}"));
            }
        } else if let Some(label) = name.strip_prefix("badge:") {
            let Some((rank, ranks)) = label.split_once('/') else {
                continue;
            };
            if let (Ok(rank), Ok(ranks)) = (rank.parse(), ranks.parse()) {
                let pips = piece(table, &format!("pips:{label}"));
                badges.insert((rank, ranks), [piece(table, name), pips]);
            }
        } else if let Some(branch) = name.strip_prefix("banner:") {
            if let Some(found) = piece(table, name) {
                banners.insert(branch.to_owned(), found);
            }
        }
    }
    let stone = std::array::from_fn(|k| piece(table, &format!("stone:{k}")));
    Ok(Sheet {
        frames,
        icons,
        badges,
        banners,
        stone,
        pixels,
        width,
        rows,
        free,
    })
}

static SHEET: LazyLock<Option<Sheet>> = LazyLock::new(|| match load() {
    Ok(sheet) => Some(sheet),
    Err(error) => {
        log::error!("path_of_craft_tree: the tree window's art is unusable: {error}");
        None
    }
});

impl Sheet {
    pub fn frame(&self, kind: Kind, state: State, compact: bool) -> Option<Piece> {
        let k = KINDS.iter().position(|known| *known == kind)?;
        self.frames[usize::from(compact)][k][state.index()]
    }

    /// The node's icon for its state; the neutral socket when the icon is not drawn.
    pub fn icon(&self, id: &str, state: State, compact: bool) -> Option<Piece> {
        let variant = match state {
            State::Locked => 1,
            State::Off => 2,
            _ => 0,
        };
        let pieces = self.icons.get(id).or_else(|| self.icons.get("?"))?;
        pieces[usize::from(compact)][variant]
    }

    /// The `rank/ranks` badge of a full frame, or the pips of a compact one.
    pub fn badge(&self, rank: u8, ranks: u8, compact: bool) -> Option<Piece> {
        self.badges.get(&(rank, ranks))?[usize::from(compact)]
    }

    pub fn banner(&self, branch: &str) -> Option<Piece> {
        self.banners.get(branch).copied()
    }
}

/// What the image slot holds now.
#[derive(Default)]
struct Upload {
    /// The tree whose zones are in the slot; none until the window first opens.
    tree: Option<Arc<Tree>>,
    look: Option<Arc<Look>>,
    /// Uploaded since the window opened; false again once it closes.
    done: bool,
    /// Whether the slot took the picture.
    ok: bool,
    /// Where the zone map sits in the picture.
    zones: Option<[f32; 2]>,
    /// The picture composed for `look`, kept so a new opening only uploads it again.
    picture: Option<Arc<Picture>>,
}

/// A composed picture: its pixels, its rows and where the zone map went.
type Picture = (Vec<u8>, u32, Option<[f32; 2]>);

static UPLOAD: LazyLock<Mutex<Upload>> = LazyLock::new(Mutex::default);

fn upload_state() -> MutexGuard<'static, Upload> {
    UPLOAD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The window's view of `tree`, worked out once per tree.
pub fn look_for(tree: &Arc<Tree>) -> Arc<Look> {
    let mut state = upload_state();
    let cached = state
        .tree
        .as_ref()
        .zip(state.look.as_ref())
        .filter(|(known, _)| Arc::ptr_eq(known, tree))
        .map(|(_, look)| look.clone());
    if let Some(look) = cached {
        return look;
    }
    let look = {
        let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::LookNew);
        Arc::new(Look::new(tree))
    };
    state.tree = Some(tree.clone());
    state.look = Some(look.clone());
    state.done = false;
    state.picture = None;
    look
}

/// The art to draw with, once the picture is in the slot; none falls back to plain shapes.
pub fn sheet() -> Option<&'static Sheet> {
    upload_state().ok.then(|| SHEET.as_ref()).flatten()
}

/// Where the zone map is in the picture, when it fitted.
pub fn zone_origin() -> Option<[f32; 2]> {
    let state = upload_state();
    state.ok.then_some(state.zones).flatten()
}

/// The picture with `look`'s zones in the free rows, a transparent pixel around them, and
/// where they went; only as many rows as are used.
fn compose(sheet: &Sheet, look: &Look) -> (Vec<u8>, u32, Option<[f32; 2]>) {
    let z = &look.zones;
    let [fx, fy, fw, fh] = sheet.free;
    let fits = z.cols > 0 && z.cols as u32 + 2 <= fw && z.rows as u32 + 2 <= fh;
    let rows = if fits {
        fy + z.rows as u32 + 2
    } else {
        sheet.rows
    };
    let stride = sheet.width as usize * 4;
    let mut image = vec![0u8; stride * rows as usize];
    image[..sheet.pixels.len()].copy_from_slice(&sheet.pixels);
    if !fits {
        return (image, rows, None);
    }
    let zones = look.zone_pixels();
    let (left, top) = (fx as usize + 1, fy as usize + 1);
    for r in 0..z.rows {
        let from = r * z.cols * 4;
        let to = (top + r) * stride + left * 4;
        image[to..to + z.cols * 4].copy_from_slice(&zones[from..from + z.cols * 4]);
    }
    (image, rows, Some([left as f32, top as f32]))
}

/// Puts the picture in the slot when the window opens or the tree changes. The slot is the
/// engine's, rebuilt with the game's other art, so it is refreshed on every opening.
pub fn on_frame(game: &mut Game<'_>) {
    if game.native_mod_screen() != Some(glue::ID) {
        upload_state().done = false;
        return;
    }
    let Some(tree) = glue::client().tree.clone() else {
        return;
    };
    let look = look_for(&tree);
    if upload_state().done {
        return;
    }
    let composing = crate::game::pct_perf::time(crate::game::pct_perf::Probe::AtlasCompose);
    let Some(sheet) = SHEET.as_ref() else {
        upload_state().done = true;
        return;
    };
    let cached = upload_state().picture.clone();
    let picture = cached.unwrap_or_else(|| {
        let picture = Arc::new(compose(sheet, &look));
        upload_state().picture = Some(picture.clone());
        picture
    });
    drop(composing);
    let (image, rows, zones) = (&picture.0, picture.1, picture.2);
    let uploading = crate::game::pct_perf::time(crate::game::pct_perf::Probe::AtlasUpload);
    let ok = game.native_mod_image(sheet.width, rows, image);
    drop(uploading);
    if !ok {
        log::warn!("path_of_craft_tree: the game has no image slot for the tree window's art");
    }
    let mut state = upload_state();
    state.done = true;
    state.ok = ok;
    state.zones = zones;
}
