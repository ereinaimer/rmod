//! Fade-to-black display-change transitions.
//!
//! [`transition`] masks a single monitor's mode switch behind a layered
//! black overlay: fade out, run the change, settle, fade in.
//! [`transition_all`] does the same around a batch of changes, covering
//! the whole virtual screen.

use super::bindings::DevmodeW;

pub(crate) const FADE_OUT_MS: u32 = 400;
pub(crate) const FADE_IN_MS: u32 = 600;
pub(crate) const FRAME_MS: u32 = 16;

/// The easing curve applied to alpha steps.
#[derive(Clone, Copy)]
pub(crate) enum Ease {
    /// Gentle start and finish; used for the fade-in.
    InOut,
    /// Immediate darkening with a slow finish; used for the fade-out.
    Out,
}

/// Alpha steps from `from` to `to` over `duration_ms`, one step per
/// `frame_ms`, eased with the given curve; the final step is always `to`.
/// At least one step is returned.
pub(crate) fn alpha_steps(
    duration_ms: u32,
    frame_ms: u32,
    from: u8,
    to: u8,
    ease: Ease,
) -> Vec<u8> {
    let count = (duration_ms / frame_ms).max(1);
    if count == 1 {
        return vec![to];
    }
    let delta = to as i32 - from as i32;
    (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1) as f64;
            let eased = match ease {
                Ease::InOut => t * t * (3.0 - 2.0 * t),
                Ease::Out => 1.0 - (1.0 - t).powi(3),
            };
            (from as f64 + delta as f64 * eased).round() as u8
        })
        .collect()
}

/// True when `current` equals `target` in width, height, refresh rate and
/// orientation.
pub(crate) fn mode_matches(current: &DevmodeW, target: &DevmodeW) -> bool {
    current.dm_pels_width == target.dm_pels_width
        && current.dm_pels_height == target.dm_pels_height
        && current.dm_display_frequency == target.dm_display_frequency
        && current.dm_display_orientation == target.dm_display_orientation
}

/// The monitor's virtual-screen rectangle as `(x, y, width, height)` from
/// a device mode's position and dimensions.
pub(crate) fn rect_of(devmode: &DevmodeW) -> (i32, i32, i32, i32) {
    (
        devmode.dm_position.x,
        devmode.dm_position.y,
        devmode.dm_pels_width as i32,
        devmode.dm_pels_height as i32,
    )
}

/// The bounding rectangle covering both `a` and `b`, in virtual-screen
/// coordinates; it may extend beyond the visible screen while the target
/// mode is larger than the current one, which is harmless for a window.
pub(crate) fn union_rect(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    let left = ax.min(bx);
    let top = ay.min(by);
    let right = (ax + aw).max(bx + bw);
    let bottom = (ay + ah).max(by + bh);
    (left, top, right - left, bottom - top)
}

use super::bindings::{
    BLACK_BRUSH, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetModuleHandleW, GetStockObject, GetSystemMetrics, HWND_TOPMOST, LWA_ALPHA, Msg, PM_REMOVE,
    PeekMessageW, RegisterClassExW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos,
    TranslateMessage, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WndClassExW,
};
use super::query;
use std::sync::OnceLock;

pub(crate) const SETTLE_CAP_MS: u64 = 1000;
pub(crate) const SETTLE_POLL_MS: u64 = 50;
pub(crate) const SETTLE_BATCH_MS: u64 = 800;

/// Nul-terminated UTF-16 class name for the overlay window class.
pub(crate) const CLASS_NAME_W: &[u16] = &[114, 109, 111, 100, 95, 102, 97, 100, 101, 0];

/// Holds the overlay opaque for this long after the device reports the
/// target mode, so DWM and the window manager finish re-scaling and
/// re-aligning windows before the reveal begins.
pub(crate) const SETTLE_GRACE_MS: u64 = 800;

/// Registers the overlay window class once per process; subsequent calls
/// return the first result.
fn register_class() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| {
        let mut wc: WndClassExW = unsafe { std::mem::zeroed() };
        wc.cb_size = std::mem::size_of::<WndClassExW>() as u32;
        wc.lpfn_wnd_proc = DefWindowProcW as *const () as usize;
        wc.h_instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        wc.h_background = unsafe { GetStockObject(BLACK_BRUSH) };
        wc.lpsz_class_name = CLASS_NAME_W.as_ptr();
        unsafe { RegisterClassExW(&wc) != 0 }
    })
}

/// A borderless topmost layered black window covering a virtual-screen
/// rectangle; destroyed on drop.
struct Overlay {
    hwnd: usize,
}

impl Overlay {
    /// Creates the overlay over `rect`; `None` when the window class or
    /// window cannot be created (the fade is best-effort). The window is
    /// created hidden at alpha 0 and shown by [`Overlay::set_rect`], so no
    /// opaque frame is ever composed before the fade begins.
    fn new(rect: (i32, i32, i32, i32)) -> Option<Overlay> {
        if !register_class() {
            return None;
        }
        let (x, y, width, height) = rect;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                CLASS_NAME_W.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                x,
                y,
                width,
                height,
                0,
                0,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        if hwnd == 0 {
            None
        } else {
            let overlay = Overlay { hwnd };
            overlay.fade_to(0);
            overlay.set_rect(rect);
            Some(overlay)
        }
    }

    /// Sets the uniform window opacity (0 transparent, 255 opaque).
    fn fade_to(&self, alpha: u8) {
        unsafe {
            SetLayeredWindowAttributes(self.hwnd, 0, alpha, LWA_ALPHA);
        }
    }

    /// Moves and resizes the overlay, keeping it topmost and visible.
    fn set_rect(&self, rect: (i32, i32, i32, i32)) {
        let (x, y, width, height) = rect;
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}

/// Pumps pending window messages so the overlay stays responsive.
fn pump_messages() {
    let mut msg: Msg = unsafe { std::mem::zeroed() };
    while unsafe { PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) } != 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Pumps messages, then sleeps `ms`.
fn sleep_pump(ms: u64) {
    pump_messages();
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

/// Animates the overlay alpha from `from` to `to`.
fn animate(overlay: &Overlay, duration_ms: u32, from: u8, to: u8, ease: Ease) {
    for alpha in alpha_steps(duration_ms, FRAME_MS, from, to, ease) {
        overlay.fade_to(alpha);
        sleep_pump(FRAME_MS as u64);
    }
}

/// Waits until the device reports `target` (mode and orientation) or the
/// settle cap expires.
fn settle(name: &str, target: &DevmodeW) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(SETTLE_CAP_MS);
    while std::time::Instant::now() < deadline {
        if query::current_mode(name).is_some_and(|mode| mode_matches(&mode, target)) {
            return;
        }
        sleep_pump(SETTLE_POLL_MS);
    }
}

/// Masks a mode change on one monitor behind a fade to black and back,
/// then returns the closure's result. Best-effort: when the overlay cannot
/// be created the change runs unmasked.
pub(crate) fn transition<T>(name: &str, target: &DevmodeW, apply: impl FnOnce() -> T) -> T {
    let Some(rect) = query::current_mode(name).map(|mode| rect_of(&mode)) else {
        return apply();
    };
    let covered = union_rect(rect, rect_of(target));
    let Some(overlay) = Overlay::new(covered) else {
        return apply();
    };
    animate(&overlay, FADE_OUT_MS, 0, 255, Ease::Out);
    let result = apply();
    settle(name, target);
    sleep_pump(SETTLE_GRACE_MS);
    if let Some(current) = query::current_mode(name) {
        let fresh = rect_of(&current);
        if fresh != rect {
            overlay.set_rect(fresh);
        }
    }
    animate(&overlay, FADE_IN_MS, 255, 0, Ease::InOut);
    result
}

/// Masks a batch of mode changes behind one fade covering the whole
/// virtual screen, then returns the closure's result. Best-effort like
/// [`transition`].
pub(crate) fn transition_all<T>(apply: impl FnOnce() -> T) -> T {
    let rect = (
        unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    );
    let Some(overlay) = Overlay::new(rect) else {
        return apply();
    };
    animate(&overlay, FADE_OUT_MS, 0, 255, Ease::Out);
    let result = apply();
    sleep_pump(SETTLE_BATCH_MS / 2);
    let fresh = (
        unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    );
    if fresh != rect {
        overlay.set_rect(fresh);
    }
    sleep_pump(SETTLE_BATCH_MS / 2);
    animate(&overlay, FADE_IN_MS, 255, 0, Ease::InOut);
    result
}

#[cfg(test)]
mod tests {
    use super::super::bindings::Pointl;
    use super::*;

    fn devmode(
        width: u32,
        height: u32,
        refresh: u32,
        orientation: u32,
        x: i32,
        y: i32,
    ) -> DevmodeW {
        let mut mode: DevmodeW = unsafe { std::mem::zeroed() };
        mode.dm_pels_width = width;
        mode.dm_pels_height = height;
        mode.dm_display_frequency = refresh;
        mode.dm_display_orientation = orientation;
        mode.dm_position = Pointl { x, y };
        mode
    }

    #[test]
    fn union_rect_covers_current_and_target() {
        assert_eq!(
            union_rect((0, 0, 1280, 720), (0, 0, 1920, 1080)),
            (0, 0, 1920, 1080)
        );
    }

    #[test]
    fn union_rect_covers_offset_rects() {
        assert_eq!(
            union_rect((0, 0, 1920, 1080), (-1920, 0, 1920, 1080)),
            (-1920, 0, 3840, 1080)
        );
    }

    #[test]
    fn alpha_steps_out_darkens_fast_then_creeps() {
        let steps = alpha_steps(400, 16, 0, 255, Ease::Out);
        assert!(
            steps[1] > 15,
            "fade-out must darken immediately, got {}",
            steps[1]
        );
        let n = steps.len();
        assert!(
            255 - steps[n - 2] < 10,
            "fade-out must creep to black, got {}",
            steps[n - 2]
        );
    }

    #[test]
    fn alpha_steps_starts_gently_and_ends_gently() {
        let steps = alpha_steps(400, 16, 0, 255, Ease::InOut);
        assert!(steps[1] < 10, "early step must be eased, got {}", steps[1]);
        let n = steps.len();
        assert!(
            255 - steps[n - 2] < 10,
            "late step must be eased, got {}",
            steps[n - 2]
        );
    }

    #[test]
    fn alpha_steps_counts_duration_over_frame() {
        assert_eq!(
            alpha_steps(FADE_OUT_MS, FRAME_MS, 0, 255, Ease::InOut).len(),
            25
        );
    }

    #[test]
    fn alpha_steps_fade_out_starts_transparent_ends_opaque() {
        let steps = alpha_steps(FADE_OUT_MS, FRAME_MS, 0, 255, Ease::InOut);
        assert_eq!(*steps.first().unwrap(), 0);
        assert_eq!(*steps.last().unwrap(), 255);
    }

    #[test]
    fn alpha_steps_fade_in_starts_opaque_ends_transparent() {
        let steps = alpha_steps(FADE_IN_MS, FRAME_MS, 255, 0, Ease::InOut);
        assert_eq!(*steps.first().unwrap(), 255);
        assert_eq!(*steps.last().unwrap(), 0);
    }

    #[test]
    fn alpha_steps_short_duration_is_single_step_to_target() {
        assert_eq!(alpha_steps(10, 16, 0, 255, Ease::InOut), vec![255]);
    }

    #[test]
    fn alpha_steps_zero_duration_is_single_step_to_target() {
        assert_eq!(alpha_steps(0, 16, 0, 255, Ease::InOut), vec![255]);
    }

    #[test]
    fn mode_matches_equal_modes() {
        let a = devmode(1920, 1080, 120, 0, 0, 0);
        let b = devmode(1920, 1080, 120, 0, 0, 0);
        assert!(mode_matches(&a, &b));
    }

    #[test]
    fn mode_matches_differs_on_width() {
        let a = devmode(1920, 1080, 120, 0, 0, 0);
        let b = devmode(2560, 1080, 120, 0, 0, 0);
        assert!(!mode_matches(&a, &b));
    }

    #[test]
    fn mode_matches_differs_on_height() {
        let a = devmode(1920, 1080, 120, 0, 0, 0);
        let b = devmode(1920, 1440, 120, 0, 0, 0);
        assert!(!mode_matches(&a, &b));
    }

    #[test]
    fn mode_matches_differs_on_refresh() {
        let a = devmode(1920, 1080, 120, 0, 0, 0);
        let b = devmode(1920, 1080, 60, 0, 0, 0);
        assert!(!mode_matches(&a, &b));
    }

    #[test]
    fn mode_matches_differs_on_orientation() {
        let a = devmode(1920, 1080, 120, 0, 0, 0);
        let b = devmode(1920, 1080, 120, 1, 0, 0);
        assert!(!mode_matches(&a, &b));
    }

    #[test]
    fn rect_of_uses_position_and_dims() {
        let mode = devmode(1920, 1080, 120, 0, -1920, 0);
        assert_eq!(rect_of(&mode), (-1920, 0, 1920, 1080));
    }

    #[test]
    fn class_name_is_nul_terminated_wide() {
        assert_eq!(
            CLASS_NAME_W,
            &[114, 109, 111, 100, 95, 102, 97, 100, 101, 0]
        );
    }
}
