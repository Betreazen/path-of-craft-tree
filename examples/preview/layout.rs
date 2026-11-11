// The preview layout. Each branch owns a wedge of the circle: its spine meanders outwards inside
// it, the ring links between neighbouring spines are laid next, and then every side cluster is
// tried at random angles, lengths and bends until one fits the wedge without touching a node or
// link already placed. Nothing is ever placed over anything else, so no link can cross another.
use super::geometry::{End, P, Rng, Scene};
use std::f64::consts::{SQRT_2, TAU};

const SEED: u64 = 0x5eed_7a3e_0fc4_af70;
/// The window's node side at scale 1, from view.rs; a node's radius is half its diagonal.
const NODE_SIZE: f64 = 22.0;
const START_RADIUS: f64 = 500.0;
const SPINE_STEP: (f64, f64) = (33.0, 40.0);
/// The part of the wedge's half width the spine's two waves may swing over.
const WAVE: [(f64, f64); 2] = [(0.14, 0.24), (0.05, 0.10)];
const WAVE_FREQUENCY: [(f64, f64); 2] = [(0.10, 0.20), (0.30, 0.55)];
const SPINE_JITTER: f64 = 5.0;
/// Spine nodes over which the swing grows from nothing, so all spines leave the start straight.
const SPINE_EASE: f64 = 3.0;
const HALF_WEDGE_DEGREES: f64 = 11.25;
/// Room kept free on each side of a wedge border, so neighbouring branches stay apart.
const WEDGE_MARGIN: f64 = 24.0;
const BIG_DISTANCE: (f64, f64) = (72.0, 135.0);
const BIG_LEAN_DEGREES: f64 = 60.0;
const CHAIN_TURN_DEGREES: f64 = 40.0;
const CHAIN_BEND_DEGREES: f64 = 24.0;
const CHAIN_WOBBLE_DEGREES: f64 = 8.0;
const FIRST_STEP: (f64, f64) = (47.0, 58.0);
const NEXT_STEP: (f64, f64) = (40.0, 50.0);
const SIDE_FLIP_CHANCE: f64 = 0.25;
const CLUSTER_TRIES: usize = 3000;
/// Fitting shapes compared per cluster; the one with the most room around it wins.
const CANDIDATES: usize = 60;
/// Room beyond this counts no more, so a cluster is not pushed far out just for space.
const ROOM_CAP: f64 = 60.0;
const OUTWARD_WEIGHT: f64 = 12.0;
/// Tries after which a cluster's side is flipped, the wedge stops binding and sizes stop shrinking.
const FLIP_EVERY: usize = 400;
const WEDGE_BINDS_FOR: usize = 2400;
const MAX_SHRINK: f64 = 0.3;
const SPINE_ATTEMPTS: usize = 200;
/// How far a bridge node sits outwards of the middle of its ring link, leaving room below it
/// for the side cluster just inside the ring.
const BRIDGE_LIFT: f64 = 36.0;

pub fn radius(kind: &str) -> f64 {
    let scale = match kind {
        "start" | "big" => 1.3,
        "upgrade" => 0.8,
        _ => 0.65,
    };
    NODE_SIZE * scale / 2.0 * SQRT_2
}

/// A branch: how many spine nodes, and for each side cluster its spine index and upgrade count.
pub struct BranchShape {
    pub spine: usize,
    pub clusters: Vec<(usize, usize)>,
}

/// A ring link from (branch, spine index) to (branch, spine index), maybe through a bridge node.
pub struct RingShape {
    pub from: (usize, usize),
    pub to: (usize, usize),
    pub bridge: bool,
}

pub struct Layout {
    pub spines: Vec<Vec<P>>,
    /// Per branch, per cluster: the big node, then its upgrades.
    pub clusters: Vec<Vec<Vec<P>>>,
    /// Per ring, the bridge node when it has one.
    pub bridges: Vec<Option<P>>,
}

pub fn lay_out(branches: &[BranchShape], rings: &[RingShape]) -> Layout {
    let count = branches.len();
    let mut seeds: Vec<u64> = (0..count as u64)
        .map(|b| SEED.wrapping_add(b.wrapping_mul(0x0001_0000_0001)))
        .collect();
    for _ in 0..SPINE_ATTEMPTS {
        let spines: Vec<Vec<P>> = branches
            .iter()
            .enumerate()
            .map(|(b, shape)| spine(axis(b, count), shape.spine, seeds[b]))
            .collect();
        let laid = frame(&spines, rings).and_then(|(scene, ids, bridges)| {
            finish(branches, spines, scene, &ids, bridges, &seeds)
        });
        match laid {
            Ok(layout) => return layout,
            Err(b) => seeds[b] = Rng::new(seeds[b]).next_u64(),
        }
    }
    panic!("no layout without crossings after {SPINE_ATTEMPTS} attempts");
}

fn axis(branch: usize, count: usize) -> f64 {
    (-90.0_f64).to_radians() + TAU * branch as f64 / count as f64
}

fn spine(axis: f64, count: usize, seed: u64) -> Vec<P> {
    let mut rng = Rng::new(seed);
    let waves: Vec<(f64, f64, f64)> = WAVE
        .iter()
        .zip(WAVE_FREQUENCY)
        .map(|(amplitude, frequency)| {
            (
                rng.range(amplitude.0, amplitude.1),
                rng.range(frequency.0, frequency.1),
                rng.range(0.0, TAU),
            )
        })
        .collect();
    let mut distance = START_RADIUS;
    (0..count)
        .map(|k| {
            if k > 0 {
                distance += rng.range(SPINE_STEP.0, SPINE_STEP.1);
            }
            let half = distance * HALF_WEDGE_DEGREES.to_radians().tan();
            let swing: f64 = waves
                .iter()
                .map(|(amplitude, frequency, phase)| {
                    amplitude * (k as f64 * frequency + phase).sin()
                })
                .sum();
            let ease = (k as f64 / SPINE_EASE).min(1.0);
            let side = ease * (swing * half + rng.range(-SPINE_JITTER, SPINE_JITTER));
            P::polar(axis + side / distance, distance)
        })
        .collect()
}

type Frame = (Scene, Vec<Vec<usize>>, Vec<Option<P>>);
/// A cluster placement being weighed: its score, spots, discs and links.
type Candidate = (f64, Vec<P>, Vec<(P, f64)>, Vec<(End, End)>);

/// Places the start, the spines and the ring links, or names a branch to lay out again.
fn frame(spines: &[Vec<P>], rings: &[RingShape]) -> Result<Frame, usize> {
    let mut scene = Scene::default();
    let start = scene
        .try_add(&[(P::new(0.0, 0.0), radius("start"))], &[])
        .expect("an empty scene")[0];
    let mut ids = Vec::with_capacity(spines.len());
    for (b, points) in spines.iter().enumerate() {
        let nodes: Vec<(P, f64)> = points.iter().map(|p| (*p, radius("small"))).collect();
        let links: Vec<(End, End)> = (0..points.len())
            .map(|k| {
                let from = if k == 0 {
                    End::Placed(start)
                } else {
                    End::New(k - 1)
                };
                (from, End::New(k))
            })
            .collect();
        ids.push(scene.try_add(&nodes, &links).ok_or(b)?);
    }
    let mut bridges = Vec::with_capacity(rings.len());
    for ring in rings {
        let (from, to) = (ids[ring.from.0][ring.from.1], ids[ring.to.0][ring.to.1]);
        let (from_point, to_point) = (scene.position(from), scene.position(to));
        let bridge = ring.bridge.then(|| {
            let middle = from_point.add(to_point).scale(0.5);
            middle.add(middle.unit().scale(BRIDGE_LIFT))
        });
        let added = match bridge {
            Some(point) => scene.try_add(
                &[(point, radius("small"))],
                &[
                    (End::Placed(from), End::New(0)),
                    (End::New(0), End::Placed(to)),
                ],
            ),
            None => scene.try_add(&[], &[(End::Placed(from), End::Placed(to))]),
        };
        added.ok_or(ring.to.0)?;
        bridges.push(bridge);
    }
    Ok((scene, ids, bridges))
}

fn finish(
    branches: &[BranchShape],
    spines: Vec<Vec<P>>,
    mut scene: Scene,
    ids: &[Vec<usize>],
    bridges: Vec<Option<P>>,
    seeds: &[u64],
) -> Result<Layout, usize> {
    let count = branches.len();
    let mut clusters = Vec::with_capacity(count);
    for (b, shape) in branches.iter().enumerate() {
        let mut rng = Rng::new(seeds[b] ^ 0xc1u64.rotate_left(40));
        let mut side = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let mut placed = Vec::with_capacity(shape.clusters.len());
        for (k, upgrades) in &shape.clusters {
            if !rng.chance(SIDE_FLIP_CHANCE) {
                side = -side;
            }
            let cluster = Cluster {
                axis: axis(b, count),
                spine: &spines[b],
                index: *k,
                spine_id: ids[b][*k],
                upgrades: *upgrades,
                side,
            };
            placed.push(cluster.place(&mut scene, &mut rng).ok_or(b)?);
        }
        clusters.push(placed);
    }
    Ok(Layout {
        spines,
        clusters,
        bridges,
    })
}

struct Cluster<'a> {
    axis: f64,
    spine: &'a [P],
    index: usize,
    spine_id: usize,
    upgrades: usize,
    side: f64,
}

impl Cluster<'_> {
    /// Tries random shapes for the cluster, keeps the roomiest of the first few that fit and adds
    /// it; None when none fits.
    fn place(&self, scene: &mut Scene, rng: &mut Rng) -> Option<Vec<P>> {
        let mut best: Option<Candidate> = None;
        let mut found = 0;
        for attempt in 0..CLUSTER_TRIES {
            if found >= CANDIDATES {
                break;
            }
            let side = if (attempt / FLIP_EVERY) % 2 == 1 {
                -self.side
            } else {
                self.side
            };
            let shrink = 1.0 - MAX_SHRINK * (attempt as f64 / CLUSTER_TRIES as f64);
            let points = self.sample(rng, side, shrink);
            let in_wedge = points.iter().all(|point| self.in_wedge(*point));
            if attempt < WEDGE_BINDS_FOR && !in_wedge {
                continue;
            }
            let nodes: Vec<(P, f64)> = points
                .iter()
                .enumerate()
                .map(|(i, p)| (*p, radius(if i == 0 { "big" } else { "upgrade" })))
                .collect();
            let links: Vec<(End, End)> = (0..points.len())
                .map(|i| {
                    let from = if i == 0 {
                        End::Placed(self.spine_id)
                    } else {
                        End::New(i - 1)
                    };
                    (from, End::New(i))
                })
                .collect();
            if !scene.fits(&nodes, &links) {
                continue;
            }
            found += 1;
            let score = self.score(scene, &points, &nodes);
            if best.as_ref().is_none_or(|(top, ..)| score > *top) {
                best = Some((score, points, nodes, links));
            }
        }
        let (_, points, nodes, links) = best?;
        scene
            .try_add(&nodes, &links)
            .expect("a shape that was checked");
        Some(points)
    }

    /// Free space around the cluster, a little extra for chains that end further out.
    fn score(&self, scene: &Scene, points: &[P], nodes: &[(P, f64)]) -> f64 {
        let here = self.spine[self.index];
        let tip = points[points.len() - 1];
        let outward = tip.sub(here).unit().dot(here.unit());
        scene.room(nodes).min(ROOM_CAP) + OUTWARD_WEIGHT * outward
    }

    fn sample(&self, rng: &mut Rng, side: f64, shrink: f64) -> Vec<P> {
        let here = self.spine[self.index];
        let ahead = self.spine[(self.index + 1).min(self.spine.len() - 1)];
        let behind = self.spine[self.index.saturating_sub(1)];
        let tangent = ahead.sub(behind).unit();
        let normal = tangent.perp().scale(side);
        let lean = rng.range(-1.0, 1.0) * BIG_LEAN_DEGREES.to_radians();
        let out = normal.scale(lean.cos()).add(tangent.scale(lean.sin()));
        let big = here.add(out.scale(rng.range(BIG_DISTANCE.0, BIG_DISTANCE.1) * shrink));
        let mut heading = out.angle() + degrees(rng, CHAIN_TURN_DEGREES);
        let bend = degrees(rng, CHAIN_BEND_DEGREES);
        let mut points = vec![big];
        for i in 0..self.upgrades {
            let (low, high) = if i == 0 { FIRST_STEP } else { NEXT_STEP };
            let step = (rng.range(low, high) * shrink).max(low * (1.0 - MAX_SHRINK / 2.0));
            let next = points[i].add(P::polar(heading, step));
            points.push(next);
            heading += bend + degrees(rng, CHAIN_WOBBLE_DEGREES);
        }
        points
    }

    fn in_wedge(&self, point: P) -> bool {
        let off = (point.angle() - self.axis + TAU / 2.0).rem_euclid(TAU) - TAU / 2.0;
        off.abs() <= HALF_WEDGE_DEGREES.to_radians() - WEDGE_MARGIN / point.length().max(1.0)
    }
}

fn degrees(rng: &mut Rng, limit: f64) -> f64 {
    rng.range(-limit, limit).to_radians()
}
