// The tree window's panning, zooming and hit testing, free of game types.

/// Low enough that the whole tree fits a small window, as the preview's "Показать всё" does.
pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 3.0;
pub const NODE_SIZE: f32 = 22.0;
/// How far the mouse may travel between press and release and still count as a click.
pub const CLICK_SLOP: f32 = 3.0;

const ZOOM_PER_STEP: f32 = 1.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub pan: [f32; 2],
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pan: [0.0, 0.0],
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub fn to_screen(self, centre: [f32; 2], world: [f32; 2]) -> [f32; 2] {
        [
            centre[0] + (world[0] + self.pan[0]) * self.zoom,
            centre[1] + (world[1] + self.pan[1]) * self.zoom,
        ]
    }

    pub fn to_world(self, centre: [f32; 2], screen: [f32; 2]) -> [f32; 2] {
        [
            (screen[0] - centre[0]) / self.zoom - self.pan[0],
            (screen[1] - centre[1]) / self.zoom - self.pan[1],
        ]
    }

    /// `delta` is the mouse's movement in GUI pixels.
    pub fn panned(self, delta: [f32; 2]) -> Camera {
        Camera {
            pan: [
                self.pan[0] + delta[0] / self.zoom,
                self.pan[1] + delta[1] / self.zoom,
            ],
            zoom: self.zoom,
        }
    }

    /// Zooms by wheel `steps`, keeping the point under `cursor` where it is.
    pub fn zoomed_at(self, centre: [f32; 2], cursor: [f32; 2], steps: f32) -> Camera {
        let zoom = (self.zoom * ZOOM_PER_STEP.powf(steps)).clamp(MIN_ZOOM, MAX_ZOOM);
        let world = self.to_world(centre, cursor);
        Camera {
            pan: [
                (cursor[0] - centre[0]) / zoom - world[0],
                (cursor[1] - centre[1]) / zoom - world[1],
            ],
            zoom,
        }
    }

    /// The side of a node drawn at `scale` of the normal size.
    pub fn node_size(self, scale: f32) -> f32 {
        NODE_SIZE * scale * self.zoom
    }

    pub fn hits(self, centre: [f32; 2], world: [f32; 2], scale: f32, cursor: [f32; 2]) -> bool {
        let [x, y] = self.to_screen(centre, world);
        let half = self.node_size(scale) / 2.0;
        (cursor[0] - x).abs() <= half && (cursor[1] - y).abs() <= half
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CENTRE: [f32; 2] = [200.0, 100.0];

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    #[test]
    fn screen_and_world_convert_back_and_forth() {
        let camera = Camera {
            pan: [12.0, -7.0],
            zoom: 1.7,
        };
        let world = [33.0, 44.0];
        assert!(close(
            camera.to_world(CENTRE, camera.to_screen(CENTRE, world)),
            world
        ));
    }

    #[test]
    fn panning_moves_a_node_by_the_mouse_delta_at_any_zoom() {
        let camera = Camera {
            pan: [0.0, 0.0],
            zoom: 2.0,
        };
        let before = camera.to_screen(CENTRE, [10.0, 10.0]);
        let after = camera.panned([6.0, -4.0]).to_screen(CENTRE, [10.0, 10.0]);
        assert!(close(after, [before[0] + 6.0, before[1] - 4.0]));
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor_still() {
        let camera = Camera {
            pan: [5.0, 9.0],
            zoom: 1.0,
        };
        let cursor = [260.0, 40.0];
        let world = camera.to_world(CENTRE, cursor);
        let zoomed = camera.zoomed_at(CENTRE, cursor, 3.0);
        assert!(zoomed.zoom > camera.zoom);
        assert!(close(zoomed.to_screen(CENTRE, world), cursor));
    }

    #[test]
    fn zoom_stays_in_its_limits() {
        let camera = Camera::default();
        assert_eq!(camera.zoomed_at(CENTRE, CENTRE, 100.0).zoom, MAX_ZOOM);
        assert_eq!(camera.zoomed_at(CENTRE, CENTRE, -100.0).zoom, MIN_ZOOM);
    }

    #[test]
    fn hit_area_follows_zoom_and_node_scale() {
        let world = [0.0, 0.0];
        let near = [CENTRE[0] + 15.0, CENTRE[1]];
        assert!(!Camera::default().hits(CENTRE, world, 1.0, near));
        assert!(Camera::default().hits(CENTRE, world, 1.5, near));
        assert!(
            Camera {
                pan: [0.0, 0.0],
                zoom: 2.0
            }
            .hits(CENTRE, world, 1.0, near)
        );
    }
}
