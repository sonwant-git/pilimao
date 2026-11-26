//! Three-window (Main / EQ / PL) magnetic docking, BFS group movement, and Windowshade engine.

use std::collections::{HashSet, VecDeque};

/// Classic Winamp 2.x base logical dimensions (1x).
pub const MAIN_WIDTH: i32 = 275;
pub const MAIN_HEIGHT: i32 = 116;
pub const EQ_WIDTH: i32 = 275;
pub const EQ_HEIGHT: i32 = 116;
pub const SHADE_HEIGHT: i32 = 14;
pub const PL_MIN_WIDTH: i32 = 275;
pub const PL_MIN_HEIGHT: i32 = 116;
pub const PL_STEP_WIDTH: i32 = 25;
pub const PL_STEP_HEIGHT: i32 = 29;
pub const DEFAULT_SNAP_DISTANCE: i32 = 10;

/// Identifier for each of the three classic Winamp windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowId {
    Main,
    Equalizer,
    Playlist,
}

impl WindowId {
    pub const ALL: [WindowId; 3] = [Self::Main, Self::Equalizer, Self::Playlist];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Main => 0,
            Self::Equalizer => 1,
            Self::Playlist => 2,
        }
    }
}

/// Screen-space integer rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl WindowRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    #[must_use]
    pub const fn right(self) -> i32 {
        self.x + self.w
    }

    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.y + self.h
    }

    /// Returns true if two windows touch along a shared horizontal or vertical edge
    /// (within `tolerance` pixels) with non-zero perpendicular overlap.
    #[must_use]
    pub fn is_docked_to(self, other: Self, tolerance: i32) -> bool {
        let horiz_touch = ((self.right() - other.x).abs() <= tolerance
            || (other.right() - self.x).abs() <= tolerance)
            && self.y < other.bottom() + tolerance
            && other.y < self.bottom() + tolerance;

        let vert_touch = ((self.bottom() - other.y).abs() <= tolerance
            || (other.bottom() - self.y).abs() <= tolerance)
            && self.x < other.right() + tolerance
            && other.x < self.right() + tolerance;

        horiz_touch || vert_touch
    }

    /// Returns true if `other` is docked directly beneath `self` (`other.y ≈ self.bottom()`).
    #[must_use]
    pub fn has_docked_below(self, other: Self, tolerance: i32) -> bool {
        (self.bottom() - other.y).abs() <= tolerance
            && self.x < other.right()
            && other.x < self.right()
    }
}

/// Per-window state inside [`DockingEngine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowState {
    pub id: WindowId,
    pub rect: WindowRect,
    /// Full (un-shaded) height in 1x logical pixels.
    pub normal_logical_height: i32,
    /// Logical width in 1x pixels.
    pub logical_width: i32,
    pub visible: bool,
    pub shaded: bool,
}

/// Pure, deterministic 3-window magnetic docking, BFS group movement, and Windowshade engine.
#[derive(Debug, Clone)]
pub struct DockingEngine {
    windows: [WindowState; 3],
    work_area: WindowRect,
    scale: u32,
    base_snap_distance: i32,
}

impl DockingEngine {
    /// Creates a new classic vertical stack (`Main` -> `EQ` -> `Playlist`) starting at `(x, y)`.
    #[must_use]
    pub fn new(origin_x: i32, origin_y: i32, scale: u32, work_area: WindowRect) -> Self {
        let s = scale.clamp(1, 3) as i32;
        let main = WindowState {
            id: WindowId::Main,
            rect: WindowRect::new(origin_x, origin_y, MAIN_WIDTH * s, MAIN_HEIGHT * s),
            normal_logical_height: MAIN_HEIGHT,
            logical_width: MAIN_WIDTH,
            visible: true,
            shaded: false,
        };
        let eq = WindowState {
            id: WindowId::Equalizer,
            rect: WindowRect::new(origin_x, main.rect.bottom(), EQ_WIDTH * s, EQ_HEIGHT * s),
            normal_logical_height: EQ_HEIGHT,
            logical_width: EQ_WIDTH,
            visible: true,
            shaded: false,
        };
        let pl = WindowState {
            id: WindowId::Playlist,
            rect: WindowRect::new(
                origin_x,
                eq.rect.bottom(),
                PL_MIN_WIDTH * s,
                PL_MIN_HEIGHT * s,
            ),
            normal_logical_height: PL_MIN_HEIGHT,
            logical_width: PL_MIN_WIDTH,
            visible: true,
            shaded: false,
        };

        Self {
            windows: [main, eq, pl],
            work_area,
            scale: scale.clamp(1, 3),
            base_snap_distance: DEFAULT_SNAP_DISTANCE,
        }
    }

    /// Returns the state of a specific window.
    #[must_use]
    pub fn state(&self, id: WindowId) -> WindowState {
        self.windows[id.index()]
    }

    /// Returns all 3 window states.
    #[must_use]
    pub fn all_states(&self) -> &[WindowState; 3] {
        &self.windows
    }

    /// Returns current integer scale factor (`1`, `2`, or `3`).
    #[must_use]
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Updates monitor work area rectangle.
    pub fn set_work_area(&mut self, work_area: WindowRect) {
        self.work_area = work_area;
    }

    /// Toggles visibility of a window.
    pub fn set_visible(&mut self, id: WindowId, visible: bool) {
        self.windows[id.index()].visible = visible;
    }

    /// Directly positions a window (e.g., when restoring saved layout from `pilimao.ini`).
    pub fn set_position(&mut self, id: WindowId, x: i32, y: i32) {
        let win = &mut self.windows[id.index()];
        win.rect.x = x;
        win.rect.y = y;
    }

    /// Changes integer scale (`1`, `2`, or `3`) anchored at `Main`'s top-left corner,
    /// scaling offsets proportionally so docked edges stay perfectly flush!
    pub fn set_scale(&mut self, new_scale: u32) -> Vec<(WindowId, WindowRect)> {
        let target = new_scale.clamp(1, 3);
        if target == self.scale {
            return self.visible_rects();
        }
        let old_s = self.scale as i32;
        let new_s = target as i32;
        let anchor_x = self.windows[WindowId::Main.index()].rect.x;
        let anchor_y = self.windows[WindowId::Main.index()].rect.y;

        for win in &mut self.windows {
            let rel_x = (win.rect.x - anchor_x) / old_s;
            let rel_y = (win.rect.y - anchor_y) / old_s;
            let logical_h = if win.shaded {
                SHADE_HEIGHT
            } else {
                win.normal_logical_height
            };
            win.rect = WindowRect::new(
                anchor_x + rel_x * new_s,
                anchor_y + rel_y * new_s,
                win.logical_width * new_s,
                logical_h * new_s,
            );
        }
        self.scale = target;
        self.visible_rects()
    }

    /// Performs Breadth-First Search (BFS) over the visible docking graph starting at `root`,
    /// returning all transitively docked window IDs (including `root`).
    #[must_use]
    pub fn connected_group_bfs(&self, root: WindowId) -> Vec<WindowId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        if !self.windows[root.index()].visible {
            return result;
        }

        visited.insert(root);
        queue.push_back(root);

        while let Some(curr) = queue.pop_front() {
            result.push(curr);
            let curr_rect = self.windows[curr.index()].rect;
            for &candidate in &WindowId::ALL {
                if visited.contains(&candidate) || !self.windows[candidate.index()].visible {
                    continue;
                }
                let cand_rect = self.windows[candidate.index()].rect;
                if curr_rect.is_docked_to(cand_rect, 2) {
                    visited.insert(candidate);
                    queue.push_back(candidate);
                }
            }
        }
        result
    }

    /// Drags `id` toward `(desired_x, desired_y)` with magnetic snapping.
    ///
    /// - Dragging `Main` moves its entire BFS-connected docked cluster together.
    /// - Dragging `Equalizer` or `Playlist` moves that individual window and magnetically snaps
    ///   against other visible windows and screen edges.
    ///
    /// Returns the list of moved `(WindowId, WindowRect)` pairs for atomic `BeginDeferWindowPos`.
    pub fn drag_window(
        &mut self,
        id: WindowId,
        desired_x: i32,
        desired_y: i32,
    ) -> Vec<(WindowId, WindowRect)> {
        let snap = self.base_snap_distance * self.scale as i32;
        let current_rect = self.windows[id.index()].rect;
        let raw_dx = desired_x - current_rect.x;
        let raw_dy = desired_y - current_rect.y;

        let moving: Vec<WindowId> = if id == WindowId::Main {
            self.connected_group_bfs(WindowId::Main)
        } else {
            vec![id]
        };

        let stationary: Vec<WindowId> = WindowId::ALL
            .into_iter()
            .filter(|w| !moving.contains(w) && self.windows[w.index()].visible)
            .collect();

        // Compute best magnetic snap adjustment across all windows in the moving cluster
        let mut best_snap_dx: Option<i32> = None;
        let mut best_snap_dy: Option<i32> = None;

        for &m_id in &moving {
            let m = self.windows[m_id.index()].rect;
            let cand = WindowRect::new(m.x + raw_dx, m.y + raw_dy, m.w, m.h);

            // 1. Snap to screen work area edges
            consider_snap(&mut best_snap_dx, self.work_area.x - cand.x, snap);
            consider_snap(
                &mut best_snap_dx,
                self.work_area.right() - cand.right(),
                snap,
            );
            consider_snap(&mut best_snap_dy, self.work_area.y - cand.y, snap);
            consider_snap(
                &mut best_snap_dy,
                self.work_area.bottom() - cand.bottom(),
                snap,
            );

            // 2. Snap to stationary windows (outer-to-outer docking + flush edge alignment)
            for &s_id in &stationary {
                let s = self.windows[s_id.index()].rect;

                let near_y = cand.y <= s.bottom() + snap && s.y <= cand.bottom() + snap;
                if near_y {
                    consider_snap(&mut best_snap_dx, s.right() - cand.x, snap);
                    consider_snap(&mut best_snap_dx, s.x - cand.right(), snap);
                    consider_snap(&mut best_snap_dx, s.x - cand.x, snap);
                    consider_snap(&mut best_snap_dx, s.right() - cand.right(), snap);
                }

                let near_x = cand.x <= s.right() + snap && s.x <= cand.right() + snap;
                if near_x {
                    consider_snap(&mut best_snap_dy, s.bottom() - cand.y, snap);
                    consider_snap(&mut best_snap_dy, s.y - cand.bottom(), snap);
                    consider_snap(&mut best_snap_dy, s.y - cand.y, snap);
                    consider_snap(&mut best_snap_dy, s.bottom() - cand.bottom(), snap);
                }
            }
        }

        let final_dx = raw_dx + best_snap_dx.unwrap_or(0);
        let final_dy = raw_dy + best_snap_dy.unwrap_or(0);

        let mut moved = Vec::with_capacity(moving.len());
        for m_id in moving {
            let win = &mut self.windows[m_id.index()];
            win.rect.x += final_dx;
            win.rect.y += final_dy;
            moved.push((m_id, win.rect));
        }
        moved
    }

    /// Toggles Windowshade mode on `id`, shifting all windows docked beneath it (found via BFS)
    /// vertically by the height delta so the stack stays tightly docked.
    pub fn toggle_windowshade(&mut self, id: WindowId) -> Vec<(WindowId, WindowRect)> {
        // Discover all windows docked below `id` via BFS BEFORE resizing `id`!
        let mut below = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(id);
        queue.push_back(id);

        while let Some(curr) = queue.pop_front() {
            let curr_rect = self.windows[curr.index()].rect;
            for &cand in &WindowId::ALL {
                if visited.contains(&cand) || !self.windows[cand.index()].visible {
                    continue;
                }
                let cand_rect = self.windows[cand.index()].rect;
                if curr_rect.has_docked_below(cand_rect, 2) {
                    visited.insert(cand);
                    below.push(cand);
                    queue.push_back(cand);
                }
            }
        }

        let s = self.scale as i32;
        let win = &mut self.windows[id.index()];
        let old_h = win.rect.h;
        win.shaded = !win.shaded;
        let new_h = if win.shaded {
            SHADE_HEIGHT * s
        } else {
            win.normal_logical_height * s
        };
        win.rect.h = new_h;
        let delta_y = new_h - old_h;

        let mut updated = vec![(id, win.rect)];
        for b_id in below {
            let b_win = &mut self.windows[b_id.index()];
            b_win.rect.y += delta_y;
            updated.push((b_id, b_win.rect));
        }
        updated
    }

    /// Resizes the Playlist window snapped to authentic Winamp tile steps (`275 + 25*k`, `116 + 29*m`).
    pub fn resize_playlist(&mut self, desired_scaled_w: i32, desired_scaled_h: i32) -> WindowRect {
        let s = self.scale as i32;
        let raw_w = (desired_scaled_w / s).max(PL_MIN_WIDTH);
        let raw_h = (desired_scaled_h / s).max(PL_MIN_HEIGHT);

        let steps_w = ((raw_w - PL_MIN_WIDTH + PL_STEP_WIDTH / 2) / PL_STEP_WIDTH).max(0);
        let steps_h = ((raw_h - PL_MIN_HEIGHT + PL_STEP_HEIGHT / 2) / PL_STEP_HEIGHT).max(0);

        let logical_w = PL_MIN_WIDTH + steps_w * PL_STEP_WIDTH;
        let logical_h = PL_MIN_HEIGHT + steps_h * PL_STEP_HEIGHT;

        let pl = &mut self.windows[WindowId::Playlist.index()];
        pl.logical_width = logical_w;
        pl.normal_logical_height = logical_h;
        pl.rect.w = logical_w * s;
        if !pl.shaded {
            pl.rect.h = logical_h * s;
        }
        pl.rect
    }

    fn visible_rects(&self) -> Vec<(WindowId, WindowRect)> {
        self.windows
            .iter()
            .filter(|w| w.visible)
            .map(|w| (w.id, w.rect))
            .collect()
    }
}

fn consider_snap(best: &mut Option<i32>, delta: i32, threshold: i32) {
    if delta.abs() <= threshold && best.is_none_or(|cur| delta.abs() < cur.abs()) {
        *best = Some(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bfs_moves_entire_docked_stack_with_main_and_snaps_to_screen() {
        let screen = WindowRect::new(0, 0, 1920, 1080);
        let mut engine = DockingEngine::new(100, 100, 1, screen);

        let group = engine.connected_group_bfs(WindowId::Main);
        assert_eq!(group.len(), 3);

        // Drag Main to (6, 8) -> within 10px snap distance of screen (0, 0)!
        let moved = engine.drag_window(WindowId::Main, 6, 8);
        assert_eq!(moved.len(), 3);
        assert_eq!(engine.state(WindowId::Main).rect.x, 0);
        assert_eq!(engine.state(WindowId::Main).rect.y, 0);
        assert_eq!(engine.state(WindowId::Equalizer).rect.y, MAIN_HEIGHT);
        assert_eq!(
            engine.state(WindowId::Playlist).rect.y,
            MAIN_HEIGHT + EQ_HEIGHT
        );
    }

    #[test]
    fn windowshade_shifts_docked_windows_below_with_zero_gap() {
        let screen = WindowRect::new(0, 0, 1920, 1080);
        let mut engine = DockingEngine::new(100, 100, 2, screen);

        // Toggle Main Windowshade at 2x scale (232px -> 28px, delta = -204px)
        let updated = engine.toggle_windowshade(WindowId::Main);
        assert_eq!(updated.len(), 3);
        assert_eq!(engine.state(WindowId::Main).rect.h, SHADE_HEIGHT * 2);
        assert_eq!(
            engine.state(WindowId::Equalizer).rect.y,
            engine.state(WindowId::Main).rect.bottom()
        );
        assert_eq!(
            engine.state(WindowId::Playlist).rect.y,
            engine.state(WindowId::Equalizer).rect.bottom()
        );

        // Un-shade restores exact original positions
        engine.toggle_windowshade(WindowId::Main);
        assert_eq!(engine.state(WindowId::Main).rect.h, MAIN_HEIGHT * 2);
        assert_eq!(
            engine.state(WindowId::Equalizer).rect.y,
            100 + MAIN_HEIGHT * 2
        );
    }

    #[test]
    fn detaching_eq_and_snapping_back_magnetically() {
        let screen = WindowRect::new(0, 0, 1920, 1080);
        let mut engine = DockingEngine::new(100, 100, 1, screen);

        // Detach EQ far to the right
        engine.drag_window(WindowId::Equalizer, 600, 100);
        assert_eq!(
            engine.connected_group_bfs(WindowId::Main),
            vec![WindowId::Main]
        );

        // Drag EQ within 7px of Main's right edge (100 + 275 = 375)
        engine.drag_window(WindowId::Equalizer, 382, 104);
        assert_eq!(engine.state(WindowId::Equalizer).rect.x, 375);
        assert_eq!(engine.state(WindowId::Equalizer).rect.y, 100);
        assert_eq!(engine.connected_group_bfs(WindowId::Main).len(), 2);
    }
}
