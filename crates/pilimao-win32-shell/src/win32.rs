//! Native Windows 10/11 borderless 3-window Win32 shell with:
//! - `SetWindowRgn` custom skin masks (`region.txt`)
//! - `BeginDeferWindowPos` atomic multi-window magnetic docking / BFS linked movement
//! - `WM_DROPFILES` drag-and-drop for audio tracks, `.m3u`/`.m3u8`/`.pls`, `.wsz`, `.eqf`
//! - 1x / 2x / 3x nearest-neighbor BGRA rendering & classic Winamp keyboard shortcuts.

use pilimao_audio_engine::OutputMode;
use pilimao_dsp::EqSettings;
use pilimao_playlist::Playlist;
use pilimao_skin_wsz::WszSkin;

use crate::ShellError;

/// Launch parameters for the native Win32 three-window shell.
#[derive(Debug, Clone)]
pub struct ShellLaunchConfig {
    pub skin: WszSkin,
    pub scale: u32,
    pub playlist: Playlist,
    pub eq: EqSettings,
    pub volume: f64,
    pub balance: f64,
    pub output_mode: OutputMode,
    pub origin_x: i32,
    pub origin_y: i32,
}

/// Runs the native Windows 10/11 borderless 3-window shell message loop.
/// On non-Windows platforms, returns [`ShellError::WindowsOnly`].
pub fn run_win32_shell(config: ShellLaunchConfig) -> Result<(), ShellError> {
    #[cfg(windows)]
    {
        win_impl::run_win32_shell(config)
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Err(ShellError::WindowsOnly)
    }
}

#[cfg(windows)]
mod win_impl {
    use std::cell::RefCell;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::ptr;
    use std::time::Duration;

    use pilimao_audio_engine::{AudioEngine, EngineConfig, PlaybackState, VisTapReader};
    use pilimao_dsp::ReplayGainInfo;
    use pilimao_playlist::{
        JumpMatcher, Playlist, PlaylistEntry, load_eqf_file, load_playlist_file,
    };
    use pilimao_skin_wsz::{
        DockingEngine, Polygon, RegionData, WindowId, WindowRect, WszSkin, load_skin,
    };
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        ALTERNATE, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, CombineRgn, CreatePolygonRgn,
        DIB_RGB_COLORS, DeleteObject, EndPaint, HRGN, InvalidateRect, PAINTSTRUCT, RGN_OR,
        SetWindowRgn, StretchDIBits,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_MENU, VK_SHIFT,
    };
    use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
    use windows::Win32::UI::WindowsAndMessaging::{
        BeginDeferWindowPos, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
        DeferWindowPos, DispatchMessageW, EndDeferWindowPos, GetCursorPos, GetMessageW, IDC_ARROW,
        LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SPI_GETWORKAREA, SW_HIDE, SW_MINIMIZE,
        SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SetTimer, ShowWindow, SystemParametersInfoW,
        TranslateMessage, WM_CHAR, WM_DESTROY, WM_DROPFILES, WM_KEYDOWN, WM_LBUTTONDBLCLK,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_TIMER, WNDCLASSW, WS_CLIPCHILDREN,
        WS_CLIPSIBLINGS, WS_EX_ACCEPTFILES, WS_POPUP,
    };
    use windows::core::{PCWSTR, w};

    use crate::ShellError;
    use crate::hit_test::{EqHit, MainHit, PlHit, hit_test_eq, hit_test_main, hit_test_playlist};
    use crate::renderer::{ShellRenderer, UiRenderState};
    use crate::shortcuts::{Modifiers, ShortcutAction, map_virtual_key};
    use crate::surface::BgraSurface;
    use crate::win32::ShellLaunchConfig;

    const TIMER_UI_TICK: usize = 1;

    struct DragState {
        window: WindowId,
        grab_offset_x: i32,
        grab_offset_y: i32,
        resizing_pl: bool,
    }

    struct ShellApp {
        hwnds: [HWND; 3],
        skin: WszSkin,
        docking: DockingEngine,
        playlist: Playlist,
        jump_matcher: JumpMatcher,
        engine: AudioEngine,
        vis_reader: Option<VisTapReader>,
        ui: UiRenderState,
        drag: Option<DragState>,
        tick_counter: u32,
    }

    thread_local! {
        static SHELL_STATE: RefCell<Option<ShellApp>> = const { RefCell::new(None) };
    }

    pub fn run_win32_shell(config: ShellLaunchConfig) -> Result<(), ShellError> {
        let mut engine = AudioEngine::start(EngineConfig {
            output_mode: config.output_mode,
            output_sample_rate: 48_000,
            null_realtime_pacing: true,
        })
        .map_err(|e| ShellError::AudioInit(e.to_string()))?;

        engine.set_volume(config.volume);
        engine.set_balance(config.balance);
        engine.set_eq(config.eq.clone());
        let vis_reader = engine.take_vis_reader();

        let work_area = query_work_area();
        let docking = DockingEngine::new(config.origin_x, config.origin_y, config.scale, work_area);

        let class_name = w!("PilimaoWin32Window");
        let hinstance =
            unsafe { GetModuleHandleW(None) }.map_err(|e| ShellError::Win32(e.to_string()))?;

        unsafe {
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);
        }

        let titles: [PCWSTR; 3] = [
            w!("Pilimao"),
            w!("Pilimao Equalizer"),
            w!("Pilimao Playlist"),
        ];
        let mut hwnds = [HWND::default(); 3];

        for id in WindowId::ALL {
            let rect = docking.state(id).rect;
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_ACCEPTFILES,
                    class_name,
                    titles[id.index()],
                    WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    None,
                    None,
                    Some(hinstance.into()),
                    None,
                )
            }
            .map_err(|e| ShellError::Win32(e.to_string()))?;

            unsafe {
                DragAcceptFiles(hwnd, true);
            }
            hwnds[id.index()] = hwnd;
        }

        let ui = UiRenderState {
            volume: config.volume,
            balance: config.balance,
            eq: config.eq,
            ..Default::default()
        };

        let app = ShellApp {
            hwnds,
            skin: config.skin,
            docking,
            playlist: config.playlist,
            jump_matcher: JumpMatcher::new(),
            engine,
            vis_reader,
            ui,
            drag: None,
            tick_counter: 0,
        };

        // Apply initial region.txt masks and show all three windows.
        apply_all_window_regions(&app);
        for &hwnd in &app.hwnds {
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
        }
        unsafe {
            SetTimer(
                Some(app.hwnds[WindowId::Main.index()]),
                TIMER_UI_TICK,
                16,
                None,
            );
        }

        SHELL_STATE.with(|slot| {
            *slot.borrow_mut() = Some(app);
        });

        // Standard Win32 message pump
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        SHELL_STATE.with(|slot| {
            *slot.borrow_mut() = None;
        });

        Ok(())
    }

    fn query_work_area() -> WindowRect {
        let mut rc = RECT::default();
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(ptr::addr_of_mut!(rc).cast()),
                Default::default(),
            )
        };
        if ok.is_ok() {
            WindowRect::new(rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top)
        } else {
            WindowRect::new(0, 0, 1920, 1080)
        }
    }

    fn window_id_from_hwnd(app: &ShellApp, hwnd: HWND) -> Option<WindowId> {
        WindowId::ALL
            .into_iter()
            .find(|&id| app.hwnds[id.index()] == hwnd)
    }

    /// Atomically repositions multiple windows in a single DWM frame via `BeginDeferWindowPos`.
    fn apply_deferred_positions(app: &ShellApp, updates: &[(WindowId, WindowRect)]) {
        if updates.is_empty() {
            return;
        }
        unsafe {
            let Ok(mut hdwp) = BeginDeferWindowPos(updates.len() as i32) else {
                return;
            };
            for &(id, rect) in updates {
                let hwnd = app.hwnds[id.index()];
                if let Ok(next) = DeferWindowPos(
                    hdwp,
                    hwnd,
                    None,
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                ) {
                    hdwp = next;
                }
            }
            let _ = EndDeferWindowPos(hdwp);
        }
    }

    /// Applies scaled `region.txt` custom polygon masks using Win32 `SetWindowRgn`.
    fn apply_window_region(app: &ShellApp, id: WindowId) {
        let hwnd = app.hwnds[id.index()];
        let state = app.docking.state(id);
        let scale = app.docking.scale();

        let polys: &[Polygon] = match (id, state.shaded) {
            (WindowId::Main, false) => &app.skin.regions.normal,
            (WindowId::Main, true) => &app.skin.regions.window_shade,
            (WindowId::Equalizer, false) => &app.skin.regions.equalizer,
            (WindowId::Equalizer, true) => &app.skin.regions.equalizer_ws,
            (WindowId::Playlist, _) => &[],
        };

        unsafe {
            if polys.is_empty() {
                SetWindowRgn(hwnd, None, true);
                return;
            }
            let scaled = RegionData::scaled_polygons(polys, scale);
            let mut combined: Option<HRGN> = None;

            for poly in &scaled {
                let pts: Vec<POINT> = poly.iter().map(|&(x, y)| POINT { x, y }).collect();
                if pts.len() < 3 {
                    continue;
                }
                let rgn = CreatePolygonRgn(&pts, ALTERNATE);
                if rgn.is_invalid() {
                    continue;
                }
                if let Some(acc) = combined {
                    CombineRgn(Some(acc), Some(acc), Some(rgn), RGN_OR);
                    let _ = DeleteObject(rgn.into());
                } else {
                    combined = Some(rgn);
                }
            }

            // Ownership of `combined` transfers to Windows upon `SetWindowRgn`.
            SetWindowRgn(hwnd, combined, true);
        }
    }

    fn apply_all_window_regions(app: &ShellApp) {
        for id in WindowId::ALL {
            apply_window_region(app, id);
        }
    }

    fn invalidate_all(app: &ShellApp) {
        for &hwnd in &app.hwnds {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
        }
    }

    fn present_surface(hwnd: HWND, surface: &BgraSurface) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: surface.width as i32,
                        biHeight: -(surface.height as i32), // Top-down BGRA DIB
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                StretchDIBits(
                    hdc,
                    0,
                    0,
                    surface.width as i32,
                    surface.height as i32,
                    0,
                    0,
                    surface.width as i32,
                    surface.height as i32,
                    Some(surface.bgra.as_ptr().cast()),
                    &bmi,
                    DIB_RGB_COLORS,
                    windows::Win32::Graphics::Gdi::SRCCOPY,
                );
            }
            let _ = EndPaint(hwnd, &ps);
        }
    }

    fn read_modifiers() -> Modifiers {
        unsafe {
            Modifiers {
                ctrl: (GetKeyState(i32::from(VK_CONTROL.0)) as u16 & 0x8000) != 0,
                alt: (GetKeyState(i32::from(VK_MENU.0)) as u16 & 0x8000) != 0,
                shift: (GetKeyState(i32::from(VK_SHIFT.0)) as u16 & 0x8000) != 0,
            }
        }
    }

    fn start_playing_current(app: &mut ShellApp) {
        if let Some(entry) = app.playlist.current_entry().cloned() {
            app.ui.title_marquee = entry.display_title();
            app.engine
                .load_file(entry.path, true, ReplayGainInfo::default());
            if let Some(next_idx) = app.playlist.peek_next_index()
                && let Some(next_entry) = app.playlist.entries().get(next_idx)
            {
                app.engine.queue_next_file(next_entry.path.clone());
            }
        }
    }

    fn handle_shortcut(app: &mut ShellApp, action: ShortcutAction, active_win: WindowId) {
        match action {
            ShortcutAction::PreviousTrack => {
                if app.playlist.advance_previous().is_some() {
                    start_playing_current(app);
                }
            }
            ShortcutAction::Play => {
                if app.playlist.current_index().is_none() && !app.playlist.is_empty() {
                    app.playlist.jump_to(0);
                    start_playing_current(app);
                } else {
                    app.engine.play();
                }
            }
            ShortcutAction::PauseToggle => app.engine.pause(),
            ShortcutAction::Stop => app.engine.stop(),
            ShortcutAction::NextTrack => {
                if app.playlist.advance_next().is_some() {
                    start_playing_current(app);
                }
            }
            ShortcutAction::SeekBackward5s => {
                let pos = app
                    .engine
                    .snapshot()
                    .position
                    .saturating_sub(Duration::from_secs(5));
                app.engine.seek(pos);
            }
            ShortcutAction::SeekForward5s => {
                let pos = app.engine.snapshot().position + Duration::from_secs(5);
                app.engine.seek(pos);
            }
            ShortcutAction::VolumeUp => {
                app.ui.volume = (app.ui.volume + 0.05).min(1.0);
                app.engine.set_volume(app.ui.volume);
            }
            ShortcutAction::VolumeDown => {
                app.ui.volume = (app.ui.volume - 0.05).max(0.0);
                app.engine.set_volume(app.ui.volume);
            }
            ShortcutAction::ToggleShuffle => {
                app.playlist.toggle_shuffle();
            }
            ShortcutAction::CycleRepeat => {
                app.playlist.cycle_repeat();
            }
            ShortcutAction::OpenJumpToFile => {
                app.ui.jump_open = !app.ui.jump_open;
                app.ui.jump_query.clear();
                app.ui.jump_matches = app.jump_matcher.search(&app.playlist, "");
            }
            ShortcutAction::CloseJumpOverlay => {
                app.ui.jump_open = false;
                app.ui.jump_query.clear();
            }
            ShortcutAction::EnqueueSelected => {
                if let Some(sel) = app.playlist.selected_index() {
                    app.playlist.toggle_queue(sel);
                }
            }
            ShortcutAction::ToggleEqualizerWindow => {
                let vis = !app.docking.state(WindowId::Equalizer).visible;
                app.docking.set_visible(WindowId::Equalizer, vis);
                app.ui.eq_visible = vis;
                unsafe {
                    let _ = ShowWindow(
                        app.hwnds[WindowId::Equalizer.index()],
                        if vis { SW_SHOW } else { SW_HIDE },
                    );
                }
            }
            ShortcutAction::TogglePlaylistWindow => {
                let vis = !app.docking.state(WindowId::Playlist).visible;
                app.docking.set_visible(WindowId::Playlist, vis);
                app.ui.pl_visible = vis;
                unsafe {
                    let _ = ShowWindow(
                        app.hwnds[WindowId::Playlist.index()],
                        if vis { SW_SHOW } else { SW_HIDE },
                    );
                }
            }
            ShortcutAction::CycleScale => {
                let next_scale = match app.docking.scale() {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                };
                let updates = app.docking.set_scale(next_scale);
                apply_deferred_positions(app, &updates);
                apply_all_window_regions(app);
            }
            ShortcutAction::ToggleWindowShade => {
                let updates = app.docking.toggle_windowshade(active_win);
                apply_deferred_positions(app, &updates);
                apply_window_region(app, active_win);
            }
        }
        invalidate_all(app);
    }

    fn handle_drop_files(app: &mut ShellApp, hdrop: HDROP) {
        let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
        let mut added_tracks = false;

        for i in 0..count {
            let len = unsafe { DragQueryFileW(hdrop, i, None) };
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            let copied = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) } as usize;
            let path = PathBuf::from(OsString::from_wide(&buf[..copied]));
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();

            match ext.as_str() {
                "wsz" => {
                    if let Ok(new_skin) = load_skin(&path) {
                        app.skin = new_skin;
                        apply_all_window_regions(app);
                    }
                }
                "eqf" => {
                    if let Ok(presets) = load_eqf_file(&path)
                        && let Some(first) = presets.first()
                    {
                        app.ui.eq.enabled = true;
                        app.ui.eq.preamp_db = first.preamp_db;
                        app.ui.eq.bands_db = first.bands_db;
                        app.engine.set_eq(app.ui.eq.clone());
                    }
                }
                "m3u" | "m3u8" | "pls" => {
                    if let Ok(entries) = load_playlist_file(&path) {
                        app.playlist.extend(entries);
                        added_tracks = true;
                    }
                }
                _ => {
                    app.playlist.push(PlaylistEntry::from_path(path));
                    added_tracks = true;
                }
            }
        }

        unsafe {
            DragFinish(hdrop);
        }

        if added_tracks && app.engine.snapshot().state == PlaybackState::Stopped {
            app.playlist.jump_to(0);
            start_playing_current(app);
        }
        invalidate_all(app);
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let mut handled = false;

        SHELL_STATE.with(|slot| {
            let mut borrow = slot.borrow_mut();
            let Some(app) = borrow.as_mut() else {
                return;
            };
            let Some(win_id) = window_id_from_hwnd(app, hwnd) else {
                return;
            };

            match msg {
                WM_PAINT => {
                    let scale = app.docking.scale();
                    let st = app.docking.state(win_id);
                    let surface = match win_id {
                        WindowId::Main => ShellRenderer::render_main(
                            &app.skin,
                            &app.ui,
                            &app.playlist,
                            st.shaded,
                            true,
                            scale,
                        ),
                        WindowId::Equalizer => ShellRenderer::render_equalizer(
                            &app.skin, &app.ui, st.shaded, true, scale,
                        ),
                        WindowId::Playlist => ShellRenderer::render_playlist(
                            &app.skin,
                            &app.ui,
                            &app.playlist,
                            (st.logical_width as u32, st.normal_logical_height as u32),
                            st.shaded,
                            true,
                            scale,
                        ),
                    };
                    present_surface(hwnd, &surface);
                    handled = true;
                }
                WM_TIMER => {
                    if wparam.0 == TIMER_UI_TICK {
                        app.tick_counter = app.tick_counter.wrapping_add(1);
                        if app.tick_counter % 10 == 0 {
                            app.ui.marquee_offset = app.ui.marquee_offset.wrapping_add(1);
                        }
                        if let Some(reader) = &mut app.vis_reader {
                            app.ui.vis = reader.latest();
                        }
                        let snap = app.engine.snapshot();
                        app.ui.playback_state = snap.state;
                        app.ui.elapsed_secs = snap.position.as_secs();
                        app.ui.duration_secs = snap.duration.map(|d| d.as_secs()).unwrap_or(0);
                        app.ui.bit_perfect = snap.bit_perfect.is_bit_perfect;
                        if let Some(fmt) = &snap.format {
                            app.ui.sample_rate_khz = fmt.sample_rate / 1000;
                            app.ui.bitrate_kbps = fmt.bitrate_kbps.unwrap_or(0);
                            app.ui.is_stereo = fmt.channels >= 2;
                        }
                        invalidate_all(app);
                        handled = true;
                    }
                }
                WM_LBUTTONDOWN => {
                    let scale = app.docking.scale() as i32;
                    let sx = (lparam.0 & 0xFFFF) as i16 as i32;
                    let sy = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let lx = sx / scale;
                    let ly = sy / scale;
                    let st = app.docking.state(win_id);

                    match win_id {
                        WindowId::Main => match hit_test_main(lx, ly, st.shaded) {
                            MainHit::TitleBar => {
                                app.drag = Some(DragState {
                                    window: win_id,
                                    grab_offset_x: sx,
                                    grab_offset_y: sy,
                                    resizing_pl: false,
                                });
                                unsafe { SetCapture(hwnd) };
                            }
                            MainHit::Minimize => unsafe {
                                let _ = ShowWindow(hwnd, SW_MINIMIZE);
                            },
                            MainHit::WindowShade => {
                                handle_shortcut(app, ShortcutAction::ToggleWindowShade, win_id);
                            }
                            MainHit::Close => unsafe { PostQuitMessage(0) },
                            MainHit::VisToggle => {
                                app.ui.vis_mode = app.ui.vis_mode.cycle();
                            }
                            MainHit::VolumeSlider(v) => {
                                app.ui.volume = v;
                                app.engine.set_volume(v);
                            }
                            MainHit::BalanceSlider(b) => {
                                app.ui.balance = b;
                                app.engine.set_balance(b);
                            }
                            MainHit::SeekBar(ratio) => {
                                if app.ui.duration_secs > 0 {
                                    let target = Duration::from_secs_f64(
                                        app.ui.duration_secs as f64 * ratio,
                                    );
                                    app.engine.seek(target);
                                }
                            }
                            MainHit::ToggleEq => {
                                handle_shortcut(app, ShortcutAction::ToggleEqualizerWindow, win_id);
                            }
                            MainHit::TogglePl => {
                                handle_shortcut(app, ShortcutAction::TogglePlaylistWindow, win_id);
                            }
                            MainHit::Prev => {
                                app.ui.pressed_transport = Some(0);
                                handle_shortcut(app, ShortcutAction::PreviousTrack, win_id);
                            }
                            MainHit::Play => {
                                app.ui.pressed_transport = Some(1);
                                handle_shortcut(app, ShortcutAction::Play, win_id);
                            }
                            MainHit::Pause => {
                                app.ui.pressed_transport = Some(2);
                                handle_shortcut(app, ShortcutAction::PauseToggle, win_id);
                            }
                            MainHit::Stop => {
                                app.ui.pressed_transport = Some(3);
                                handle_shortcut(app, ShortcutAction::Stop, win_id);
                            }
                            MainHit::Next => {
                                app.ui.pressed_transport = Some(4);
                                handle_shortcut(app, ShortcutAction::NextTrack, win_id);
                            }
                            MainHit::Eject => {
                                app.ui.pressed_transport = Some(5);
                            }
                            MainHit::ToggleShuffle => {
                                handle_shortcut(app, ShortcutAction::ToggleShuffle, win_id);
                            }
                            MainHit::CycleRepeat => {
                                handle_shortcut(app, ShortcutAction::CycleRepeat, win_id);
                            }
                            MainHit::None => {}
                        },
                        WindowId::Equalizer => match hit_test_eq(lx, ly, st.shaded) {
                            EqHit::TitleBar => {
                                app.drag = Some(DragState {
                                    window: win_id,
                                    grab_offset_x: sx,
                                    grab_offset_y: sy,
                                    resizing_pl: false,
                                });
                                unsafe { SetCapture(hwnd) };
                            }
                            EqHit::WindowShade => {
                                handle_shortcut(app, ShortcutAction::ToggleWindowShade, win_id);
                            }
                            EqHit::Close => {
                                handle_shortcut(app, ShortcutAction::ToggleEqualizerWindow, win_id);
                            }
                            EqHit::ToggleOn => {
                                app.ui.eq.enabled = !app.ui.eq.enabled;
                                app.engine.set_eq(app.ui.eq.clone());
                            }
                            EqHit::PreampSlider(db) => {
                                app.ui.eq.preamp_db = db;
                                app.engine.set_eq(app.ui.eq.clone());
                            }
                            EqHit::BandSlider { band, gain_db } => {
                                app.ui.eq.bands_db[band] = gain_db;
                                app.engine.set_eq(app.ui.eq.clone());
                            }
                            EqHit::None => {}
                        },
                        WindowId::Playlist => match hit_test_playlist(
                            lx,
                            ly,
                            st.logical_width,
                            st.normal_logical_height,
                            st.shaded,
                            app.ui.pl_scroll_offset,
                        ) {
                            PlHit::TitleBar => {
                                app.drag = Some(DragState {
                                    window: win_id,
                                    grab_offset_x: sx,
                                    grab_offset_y: sy,
                                    resizing_pl: false,
                                });
                                unsafe { SetCapture(hwnd) };
                            }
                            PlHit::ResizeCorner => {
                                app.drag = Some(DragState {
                                    window: win_id,
                                    grab_offset_x: sx,
                                    grab_offset_y: sy,
                                    resizing_pl: true,
                                });
                                unsafe { SetCapture(hwnd) };
                            }
                            PlHit::WindowShade => {
                                handle_shortcut(app, ShortcutAction::ToggleWindowShade, win_id);
                            }
                            PlHit::Close => {
                                handle_shortcut(app, ShortcutAction::TogglePlaylistWindow, win_id);
                            }
                            PlHit::TrackRow(idx) => {
                                app.playlist.set_selected(Some(idx));
                            }
                            PlHit::None => {}
                        },
                    }
                    invalidate_all(app);
                    handled = true;
                }
                WM_LBUTTONDBLCLK => {
                    if win_id == WindowId::Playlist {
                        let scale = app.docking.scale() as i32;
                        let lx = ((lparam.0 & 0xFFFF) as i16 as i32) / scale;
                        let ly = (((lparam.0 >> 16) & 0xFFFF) as i16 as i32) / scale;
                        let st = app.docking.state(win_id);
                        if let PlHit::TrackRow(idx) = hit_test_playlist(
                            lx,
                            ly,
                            st.logical_width,
                            st.normal_logical_height,
                            st.shaded,
                            app.ui.pl_scroll_offset,
                        ) && app.playlist.jump_to(idx).is_some()
                        {
                            start_playing_current(app);
                            invalidate_all(app);
                        }
                        handled = true;
                    }
                }
                WM_MOUSEMOVE => {
                    if let Some(drag) = &app.drag {
                        let mut pt = POINT::default();
                        if unsafe { GetCursorPos(&mut pt) }.is_ok() {
                            if drag.resizing_pl {
                                let cur = app.docking.state(WindowId::Playlist).rect;
                                let new_rect = app.docking.resize_playlist(
                                    (pt.x - cur.x).max(275),
                                    (pt.y - cur.y).max(116),
                                );
                                apply_deferred_positions(app, &[(WindowId::Playlist, new_rect)]);
                            } else {
                                let updates = app.docking.drag_window(
                                    drag.window,
                                    pt.x - drag.grab_offset_x,
                                    pt.y - drag.grab_offset_y,
                                );
                                apply_deferred_positions(app, &updates);
                            }
                        }
                        handled = true;
                    }
                }
                WM_LBUTTONUP => {
                    app.ui.pressed_transport = None;
                    if app.drag.take().is_some() {
                        let _ = unsafe { ReleaseCapture() };
                    }
                    invalidate_all(app);
                    handled = true;
                }
                WM_DROPFILES => {
                    handle_drop_files(app, HDROP(wparam.0 as *mut _));
                    handled = true;
                }
                WM_KEYDOWN => {
                    let vk = wparam.0 as u32;
                    let mods = read_modifiers();
                    if app.ui.jump_open {
                        if vk == 0x0D {
                            // Enter -> jump to top fuzzy match
                            if let Some(top) = app.ui.jump_matches.first()
                                && app.playlist.jump_to(top.playlist_index).is_some()
                            {
                                start_playing_current(app);
                            }
                            app.ui.jump_open = false;
                            app.ui.jump_query.clear();
                            invalidate_all(app);
                            handled = true;
                        } else if vk == 0x08 {
                            // Backspace
                            app.ui.jump_query.pop();
                            app.ui.jump_matches =
                                app.jump_matcher.search(&app.playlist, &app.ui.jump_query);
                            invalidate_all(app);
                            handled = true;
                        } else if vk == 0x1B {
                            handle_shortcut(app, ShortcutAction::CloseJumpOverlay, win_id);
                            handled = true;
                        }
                    } else if let Some(action) = map_virtual_key(vk, mods) {
                        handle_shortcut(app, action, win_id);
                        handled = true;
                    }
                }
                WM_CHAR => {
                    if app.ui.jump_open
                        && let Some(ch) = char::from_u32(wparam.0 as u32)
                        && !ch.is_control()
                    {
                        app.ui.jump_query.push(ch);
                        app.ui.jump_matches =
                            app.jump_matcher.search(&app.playlist, &app.ui.jump_query);
                        invalidate_all(app);
                        handled = true;
                    }
                }
                WM_DESTROY => {
                    if win_id == WindowId::Main {
                        unsafe { PostQuitMessage(0) };
                    }
                    handled = true;
                }
                _ => {}
            }
        });

        if handled {
            LRESULT(0)
        } else {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
