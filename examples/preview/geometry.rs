// Points, segment tests and a scene of placed nodes and links that refuses anything that would
// overlap a node, cross a link or run a link over a node it does not connect.

/// Room left between the edges of two nodes.
const NODE_GAP: f64 = 10.0;
/// Room left between a link and a node it does not connect.
const LINK_CLEARANCE: f64 = 7.0;
/// Room left between two links that share no node.
const LINK_GAP: f64 = 5.0;

/// A placed node: where it is and how much room it keeps around it.
type Disc = (P, f64);
/// What a cluster adds when it fits: its discs and its links by node index.
type Added = (Vec<Disc>, Vec<(usize, usize)>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P {
    pub x: f64,
    pub y: f64,
}

impl P {
    pub fn new(x: f64, y: f64) -> P {
        P { x, y }
    }

    pub fn polar(angle: f64, length: f64) -> P {
        P::new(angle.cos() * length, angle.sin() * length)
    }

    pub fn add(self, other: P) -> P {
        P::new(self.x + other.x, self.y + other.y)
    }

    pub fn sub(self, other: P) -> P {
        P::new(self.x - other.x, self.y - other.y)
    }

    pub fn scale(self, factor: f64) -> P {
        P::new(self.x * factor, self.y * factor)
    }

    pub fn dot(self, other: P) -> f64 {
        self.x * other.x + self.y * other.y
    }

    pub fn cross(self, other: P) -> f64 {
        self.x * other.y - self.y * other.x
    }

    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn unit(self) -> P {
        self.scale(1.0 / self.length().max(1e-9))
    }

    /// The same vector turned a quarter to the left.
    pub fn perp(self) -> P {
        P::new(-self.y, self.x)
    }

    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }
}

pub fn point_segment_distance(point: P, a: P, b: P) -> f64 {
    let ab = b.sub(a);
    let t = (point.sub(a).dot(ab) / ab.dot(ab).max(1e-9)).clamp(0.0, 1.0);
    point.sub(a.add(ab.scale(t))).length()
}

/// Whether two segments touch anywhere, end points included.
pub fn segments_touch(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| q.sub(p).cross(r.sub(p));
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return true;
    }
    point_segment_distance(a, c, d) < 1e-6
        || point_segment_distance(b, c, d) < 1e-6
        || point_segment_distance(c, a, b) < 1e-6
        || point_segment_distance(d, a, b) < 1e-6
}

pub fn segment_distance(a: P, b: P, c: P, d: P) -> f64 {
    if segments_touch(a, b, c, d) {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

/// One end of a link that is being tried: a node already in the scene or one of the new ones.
#[derive(Debug, Clone, Copy)]
pub enum End {
    Placed(usize),
    New(usize),
}

#[derive(Default)]
pub struct Scene {
    nodes: Vec<(P, f64)>,
    links: Vec<(usize, usize)>,
}

impl Scene {
    pub fn position(&self, id: usize) -> P {
        self.nodes[id].0
    }

    /// Adds the nodes and links when they fit, returning the ids given to the new nodes.
    pub fn try_add(&mut self, nodes: &[(P, f64)], links: &[(End, End)]) -> Option<Vec<usize>> {
        let first = self.nodes.len();
        let (all, links) = self.check(nodes, links)?;
        self.nodes = all;
        self.links.extend(links);
        Some((first..self.nodes.len()).collect())
    }

    pub fn fits(&self, nodes: &[(P, f64)], links: &[(End, End)]) -> bool {
        self.check(nodes, links).is_some()
    }

    /// The least free space around the given nodes: the distance from each node's edge to the
    /// nearest edge of a placed node or to a placed link.
    pub fn room(&self, nodes: &[(P, f64)]) -> f64 {
        nodes
            .iter()
            .map(|(point, radius)| {
                let to_nodes = self
                    .nodes
                    .iter()
                    .map(|(other, other_radius)| point.sub(*other).length() - other_radius)
                    .fold(f64::INFINITY, f64::min);
                let to_links = self
                    .links
                    .iter()
                    .map(|(a, b)| {
                        point_segment_distance(*point, self.nodes[*a].0, self.nodes[*b].0)
                    })
                    .fold(f64::INFINITY, f64::min);
                to_nodes.min(to_links) - radius
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// All nodes and the new links resolved to ids when the new ones fit, None otherwise.
    fn check(&self, nodes: &[(P, f64)], links: &[(End, End)]) -> Option<Added> {
        let first = self.nodes.len();
        let resolve = |end: End| match end {
            End::Placed(id) => id,
            End::New(index) => first + index,
        };
        let links: Vec<(usize, usize)> = links
            .iter()
            .map(|(a, b)| (resolve(*a), resolve(*b)))
            .collect();
        let mut all = self.nodes.clone();
        all.extend_from_slice(nodes);
        let fits = (first..all.len()).all(|id| node_fits(&all, id))
            && links.iter().enumerate().all(|(index, link)| {
                link_fits(&all, *link)
                    && self
                        .links
                        .iter()
                        .chain(&links[..index])
                        .all(|other| links_apart(&all, *link, *other))
            })
            && self
                .links
                .iter()
                .all(|link| (first..all.len()).all(|id| clear_of(&all, *link, id)));
        fits.then_some((all, links))
    }
}

fn node_fits(all: &[(P, f64)], id: usize) -> bool {
    let (point, radius) = all[id];
    all[..id]
        .iter()
        .all(|(other, other_radius)| point.sub(*other).length() >= radius + other_radius + NODE_GAP)
}

fn clear_of(all: &[(P, f64)], (a, b): (usize, usize), id: usize) -> bool {
    if id == a || id == b {
        return true;
    }
    let (point, radius) = all[id];
    point_segment_distance(point, all[a].0, all[b].0) >= radius + LINK_CLEARANCE
}

fn link_fits(all: &[(P, f64)], link: (usize, usize)) -> bool {
    (0..all.len()).all(|id| clear_of(all, link, id))
}

fn links_apart(all: &[(P, f64)], (a, b): (usize, usize), (c, d): (usize, usize)) -> bool {
    if a == c || a == d || b == c || b == d {
        return true;
    }
    segment_distance(all[a].0, all[b].0, all[c].0, all[d].0) >= LINK_GAP
}

/// SplitMix64: small, fast and the same on every machine.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    pub fn chance(&mut self, probability: f64) -> bool {
        self.unit() < probability
    }
}
