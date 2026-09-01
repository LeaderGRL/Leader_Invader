use leader_core::Rect;

const MIN_CAMERA_EXTENT: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportState {
    camera: Rect,
    world: Rect,
}

impl ViewportState {
    #[must_use]
    pub fn new(world: Rect, initial: Rect) -> Self {
        let mut state = Self {
            camera: initial,
            world,
        };
        state.clamp_camera();
        state
    }

    #[must_use]
    pub const fn camera(&self) -> Rect {
        self.camera
    }

    pub fn fit(&mut self, bounds: Rect) {
        self.camera = bounds;
        self.clamp_camera();
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        self.camera.x += dx;
        self.camera.y += dy;
        self.clamp_camera();
    }

    pub fn zoom_at(&mut self, anchor_x: f32, anchor_y: f32, factor: f32) -> bool {
        if !anchor_x.is_finite()
            || !anchor_y.is_finite()
            || !factor.is_finite()
            || factor <= 0.0
        {
            return false;
        }

        let old = self.camera;
        let new_w = (old.w / factor).clamp(MIN_CAMERA_EXTENT, self.world.w);
        let new_h = (old.h / factor).clamp(MIN_CAMERA_EXTENT, self.world.h);
        if (new_w - old.w).abs() < f32::EPSILON && (new_h - old.h).abs() < f32::EPSILON {
            return false;
        }

        let u = if old.w > 0.0 {
            ((anchor_x - old.x) / old.w).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let v = if old.h > 0.0 {
            ((anchor_y - old.y) / old.h).clamp(0.0, 1.0)
        } else {
            0.5
        };

        self.camera = Rect::new(anchor_x - u * new_w, anchor_y - v * new_h, new_w, new_h);
        self.clamp_camera();
        true
    }

    fn clamp_camera(&mut self) {
        self.camera.w = self
            .camera
            .w
            .max(MIN_CAMERA_EXTENT)
            .min(self.world.w.max(MIN_CAMERA_EXTENT));
        self.camera.h = self
            .camera
            .h
            .max(MIN_CAMERA_EXTENT)
            .min(self.world.h.max(MIN_CAMERA_EXTENT));

        let max_x = self.world.x + self.world.w - self.camera.w;
        let max_y = self.world.y + self.world.h - self.camera.h;
        self.camera.x = self.camera.x.clamp(self.world.x, max_x.max(self.world.x));
        self.camera.y = self.camera.y.clamp(self.world.y, max_y.max(self.world.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_stays_inside_world_bounds() {
        let world = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let mut state = ViewportState::new(world, Rect::new(100.0, 100.0, 400.0, 300.0));
        state.pan(900.0, 900.0);
        assert_eq!(state.camera(), Rect::new(600.0, 500.0, 400.0, 300.0));
        state.pan(-900.0, -900.0);
        assert_eq!(state.camera(), Rect::new(0.0, 0.0, 400.0, 300.0));
    }

    #[test]
    fn zoom_keeps_world_anchor_at_the_same_normalized_position() {
        let world = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let mut state = ViewportState::new(world, Rect::new(100.0, 100.0, 400.0, 300.0));
        assert!(state.zoom_at(300.0, 250.0, 2.0));
        assert_eq!(state.camera(), Rect::new(200.0, 175.0, 200.0, 150.0));
    }

    #[test]
    fn invalid_zoom_input_does_not_mutate_camera() {
        let world = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let mut state = ViewportState::new(world, Rect::new(100.0, 100.0, 400.0, 300.0));
        let before = state.camera();
        assert!(!state.zoom_at(200.0, 200.0, 0.0));
        assert_eq!(state.camera(), before);
    }
}
