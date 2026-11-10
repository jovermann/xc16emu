// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Desktop window and headless capture commands for the Rust emulator.

mod audio_output;

use audio_output::AudioOutput;
use minifb::{Key, MenuItemHandle, MouseButton, MouseMode, Scale, Window, WindowOptions};
#[cfg(not(target_os = "macos"))]
use minifb::{Menu, MenuHandle};
use std::{
    collections::VecDeque,
    env, fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use xc16emu::{matrix_for_char, Machine, Palette, PAL_TED_TICKS_PER_SECOND};

/// One-pixel outline, one-pixel inset, seven-pixel font, and bottom inset.
const STATUS_HEIGHT: usize = 11;
const TOOLBAR_HEIGHT: usize = 27;
const COLOR_DIALOG_WIDTH: usize = 360;
const COLOR_DIALOG_HEIGHT: usize = 260;
const SLIDER_START: usize = 20;
const SLIDER_END: usize = 340;
const RESET_ID: usize = 1;
const FAST_MODE_ID: usize = 2;
const STATUS_ID: usize = 3;
const BIG_BORDER_ID: usize = 4;
const MEDIUM_BORDER_ID: usize = 5;
const COMPACT_BORDER_ID: usize = 6;
const ZOOM_1_ID: usize = 7;
const ZOOM_2_ID: usize = 8;
const ZOOM_4_ID: usize = 9;
const SOUND_ID: usize = 10;
const JOYSTICK_HELP_ID: usize = 11;
const PASTE_ID: usize = 12;
const DELETE_LINE_ID: usize = 13;
const GLYPH_TABLE_ID: usize = 14;
const RUN_STOP_ID: usize = 15;
const C16_PALETTE_ID: usize = 16;
const LEGACY_PALETTE_ID: usize = 17;
const C16_PALETTE_125_ID: usize = 18;
const C16_PALETTE_150_ID: usize = 19;
const PALETTE_CHOICES: [(Palette, &str, usize); 4] = [
    (Palette::C16Colors, "Palette: C16colors", C16_PALETTE_ID),
    (
        Palette::C16Colors125,
        "Palette: C16colors +25%",
        C16_PALETTE_125_ID,
    ),
    (
        Palette::C16Colors150,
        "Palette: C16colors +50%",
        C16_PALETTE_150_ID,
    ),
    (Palette::Legacy, "Palette: Legacy", LEGACY_PALETTE_ID),
];

/// Keep the emulated CPU and audio producer running while AppKit tracks menus.
struct EmulatorWorker {
    running: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl EmulatorWorker {
    /// Start one independent PAL frame producer.
    /// Arguments: machine and audio are shared with the UI; input carries
    /// live keyboard and speed-mode state; speed is the paced rate.
    /// Returns: a worker that stops and joins when dropped.
    fn start(
        machine: Arc<Mutex<Machine>>,
        audio: Arc<Mutex<Option<AudioOutput>>>,
        input: Arc<WorkerInput>,
        speed: f64,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let error = Arc::new(Mutex::new(None));
        let thread_running = Arc::clone(&running);
        let thread_error = Arc::clone(&error);
        let thread = std::thread::spawn(move || {
            let mut next_frame = Instant::now();
            let mut was_fast = input.fast_mode.load(Ordering::Relaxed);
            while thread_running.load(Ordering::Relaxed) {
                let fast = input.fast_mode.load(Ordering::Relaxed);
                if fast != was_fast {
                    next_frame = Instant::now();
                    was_fast = fast;
                }
                let (ticks, samples) = {
                    let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
                    let before = machine.ted.ticks;
                    let mut paste = input.paste.lock().unwrap_or_else(|e| e.into_inner());
                    paste.step(&mut machine);
                    let queued_shift = paste
                        .active
                        .is_some_and(|(_, shifted)| shifted && (1..=2).contains(&paste.phase));
                    drop(paste);
                    machine.set_key(15, input.host_shift.load(Ordering::Relaxed) || queued_shift);
                    let stop_frames = input
                        .run_stop_frames
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                            Some(count.saturating_sub(1))
                        })
                        .unwrap_or(0);
                    machine.set_key(
                        63,
                        stop_frames > 0 || input.run_stop_held.load(Ordering::Relaxed),
                    );
                    if let Err(message) = machine.run_frames(1) {
                        *thread_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
                        break;
                    }
                    if fast {
                        let keys = input.held_keys.lock().unwrap_or_else(|e| e.into_inner());
                        release_fast_mode_keys(&mut machine, keys.iter().copied());
                    }
                    (machine.ted.ticks - before, machine.drain_audio_samples())
                };
                if let Some(output) = audio.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    output.push(samples);
                }
                if fast {
                    next_frame = Instant::now();
                    std::thread::yield_now();
                } else {
                    next_frame += Duration::from_secs_f64(
                        ticks as f64 / PAL_TED_TICKS_PER_SECOND as f64 / speed,
                    );
                    if let Some(wait) = next_frame.checked_duration_since(Instant::now()) {
                        std::thread::sleep(wait);
                    } else {
                        next_frame = Instant::now();
                    }
                }
            }
        });
        Self {
            running,
            error,
            thread: Some(thread),
        }
    }

    /// Report an emulation failure to the window loop.
    /// Arguments: self owns the error slot. Returns: an error, if one occurred.
    fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for EmulatorWorker {
    /// Stop and join the producer before shared machine and audio state go away.
    /// Arguments: self owns the thread. Returns: nothing.
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Characters waiting to be delivered through the C16 keyboard matrix.
#[derive(Default)]
struct PasteQueue {
    pending: VecDeque<(u8, bool)>,
    active: Option<(u8, bool)>,
    phase: u8,
}

impl PasteQueue {
    /// Queue one C16 matrix key, for example an editor-menu shortcut.
    /// Arguments: index is the keyboard-matrix position. Returns: nothing.
    fn append_key(&mut self, index: u8) {
        self.pending.push_back((index, false));
    }

    /// Parse BASIC source from the clipboard and append complete input lines.
    /// Arguments: text is clipboard UTF-8. Returns: an unsupported-character error or success.
    fn append(&mut self, text: &str) -> Result<(), String> {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut keys = VecDeque::new();
        for line in normalized.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("```") {
                continue;
            }
            for character in line.chars().chain(std::iter::once('\n')) {
                keys.push_back(matrix_for_char(character).ok_or_else(|| {
                    format!("clipboard contains unsupported character {character:?}")
                })?);
            }
        }
        self.pending.extend(keys);
        Ok(())
    }

    /// Drive one frame of a pasted key, holding and releasing each for two frames.
    /// Arguments: machine receives matrix changes. Returns: nothing.
    fn step(&mut self, machine: &mut Machine) {
        if self.active.is_none() {
            self.active = self.pending.pop_front();
            self.phase = 0;
        }
        let Some((key, shift)) = self.active else {
            return;
        };
        if self.phase == 0 {
            if shift {
                machine.set_key(15, true);
            }
            machine.set_key(key, true);
        } else if self.phase == 2 {
            machine.set_key(key, false);
            if shift {
                machine.set_key(15, false);
            }
        }
        self.phase += 1;
        if self.phase == 4 {
            self.active = None;
        }
    }

    /// Discard pending text and release any currently held emulated key.
    /// Arguments: machine receives matrix changes. Returns: nothing.
    fn clear(&mut self, machine: &mut Machine) {
        if let Some((key, shift)) = self.active.take() {
            machine.set_key(key, false);
            if shift {
                machine.set_key(15, false);
            }
        }
        self.pending.clear();
        self.phase = 0;
    }
}

/// UI inputs consumed at emulated frame boundaries, including during a menu.
#[derive(Default)]
struct WorkerInput {
    paste: Mutex<PasteQueue>,
    held_keys: Mutex<Vec<Key>>,
    fast_mode: AtomicBool,
    host_shift: AtomicBool,
    run_stop_frames: AtomicU8,
    run_stop_held: AtomicBool,
}

/// The border widths are measured in emulator pixels on every side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorderSize {
    Big,
    Medium,
    Compact,
}

impl BorderSize {
    /// Convert a border size choice into emulator pixels.
    /// Arguments: self is the border size. Returns: pixels on each side.
    fn pixels(self) -> usize {
        match self {
            Self::Big => 32,
            Self::Medium => 16,
            Self::Compact => 8,
        }
    }

    /// Compute the full picture dimensions including borders.
    /// Arguments: self is the border size. Returns: width and height in emulator pixels.
    fn dimensions(self) -> (usize, usize) {
        let border = self.pixels();
        (320 + border * 2, 200 + border * 2)
    }
}

/// User choices that are saved between desktop launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DesktopSettings {
    zoom: u8,
    border: BorderSize,
    palette: Palette,
    brightness: i16,
    contrast: u16,
    show_status: bool,
    show_joystick_help: bool,
    fast_mode: bool,
    sound_enabled: bool,
}

impl Default for DesktopSettings {
    /// Choose initial desktop display and sound settings.
    /// Arguments: none. Returns: default desktop settings.
    fn default() -> Self {
        Self {
            zoom: 2,
            border: BorderSize::Big,
            palette: Palette::default(),
            brightness: 0,
            contrast: 100,
            show_status: true,
            show_joystick_help: false,
            fast_mode: false,
            sound_enabled: true,
        }
    }
}

impl DesktopSettings {
    /// Read the small subset of TOML used by this file. Invalid or unknown
    /// values leave their defaults in place, so hand edits cannot break boot.
    /// Arguments: input is TOML text. Returns: settings with defaults for missing values.
    fn parse_toml(input: &str) -> Self {
        let mut settings = Self::default();
        for line in input.lines() {
            let Some((key, value)) = line.split('#').next().and_then(|s| s.split_once('=')) else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "zoom" => {
                    if let Ok(zoom) = value.parse::<u8>() {
                        if [1, 2, 4].contains(&zoom) {
                            settings.zoom = zoom;
                        }
                    }
                }
                "border" => {
                    settings.border = match value.trim_matches('"') {
                        "compact" => BorderSize::Compact,
                        "medium" => BorderSize::Medium,
                        "big" => BorderSize::Big,
                        _ => settings.border,
                    };
                }
                "palette" => {
                    if let Some(palette) = Palette::parse(value.trim_matches('"')) {
                        settings.palette = palette;
                    }
                }
                "brightness" => {
                    if let Ok(brightness) = value.parse::<i16>() {
                        if (-100..=100).contains(&brightness) {
                            settings.brightness = brightness;
                        }
                    }
                }
                "contrast" => {
                    if let Ok(contrast) = value.parse::<u16>() {
                        if (50..=200).contains(&contrast) {
                            settings.contrast = contrast;
                        }
                    }
                }
                "status_bar" => match value {
                    "true" => settings.show_status = true,
                    "false" => settings.show_status = false,
                    _ => {}
                },
                "joystick_help" => match value {
                    "true" => settings.show_joystick_help = true,
                    "false" => settings.show_joystick_help = false,
                    _ => {}
                },
                "fast_mode" | "free_run" => match value {
                    "true" => settings.fast_mode = true,
                    "false" => settings.fast_mode = false,
                    _ => {}
                },
                "sound" => match value {
                    "true" => settings.sound_enabled = true,
                    "false" => settings.sound_enabled = false,
                    _ => {}
                },
                _ => {}
            }
        }
        settings
    }

    /// Serialize desktop choices for the settings file.
    /// Arguments: self is the settings value. Returns: TOML text.
    fn to_toml(self) -> String {
        let border = match self.border {
            BorderSize::Big => "big",
            BorderSize::Medium => "medium",
            BorderSize::Compact => "compact",
        };
        format!(
            "zoom = {}\nborder = \"{}\"\npalette = \"{}\"\nbrightness = {}\ncontrast = {}\nstatus_bar = {}\njoystick_help = {}\nfast_mode = {}\nsound = {}\n",
            self.zoom, border, self.palette.name(), self.brightness, self.contrast, self.show_status, self.show_joystick_help, self.fast_mode, self.sound_enabled
        )
    }
}

/// Apply display brightness and contrast after palette lookup.
/// Arguments: rgb is one packed palette color; brightness is -100..100;
/// contrast is 50..200 percent. Returns: adjusted packed RGB, clipped to 8 bits.
fn adjust_rgb(rgb: u32, brightness: i16, contrast: u16) -> u32 {
    let channel = |shift| {
        let source = ((rgb >> shift) & 255u32) as i32;
        let contrasted = 128 + (source - 128) * contrast as i32 / 100;
        (contrasted * (100 + brightness as i32) / 100).clamp(0, 255) as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// Locate the per-user settings file from HOME.
/// Arguments: none. Returns: path or an unset-HOME error.
fn settings_path() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".xc16emu.toml"))
        .ok_or("HOME is not set; cannot locate ~/.xc16emu.toml".to_string())
}

/// Load settings or use defaults when unavailable.
/// Arguments: path is the settings-file location. Returns: parsed or default settings.
fn load_settings(path: &std::path::Path) -> DesktopSettings {
    match fs::read_to_string(path) {
        Ok(input) => DesktopSettings::parse_toml(&input),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DesktopSettings::default(),
        Err(error) => {
            eprintln!("xc16emu: could not read {}: {error}", path.display());
            DesktopSettings::default()
        }
    }
}

/// Write the current desktop settings and report I/O failures.
/// Arguments: path is the file location; settings is the value to save. Returns: nothing.
fn save_settings(path: &std::path::Path, settings: DesktopSettings) {
    if let Err(error) = fs::write(path, settings.to_toml()) {
        eprintln!("xc16emu: could not save {}: {error}", path.display());
    }
}

/// Give a native macOS menu item a real checkmark. minifb exposes its Cocoa
/// menu-item pointer but does not provide an API for changing its state.
#[cfg(target_os = "macos")]
/// Update a menu checkmark on the supported native backend.
/// Arguments: item is the menu handle; checked is the desired state. Returns: nothing.
fn set_menu_checked(item: MenuItemHandle, checked: bool) {
    use std::ffi::{c_char, c_void};
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSImageNameMenuOnStateTemplate: *mut c_void;
    }
    // objc_msgSend is intentionally called with the signatures of the
    // Objective-C methods selected below.
    #[allow(clashing_extern_declarations)]
    #[link(name = "objc")]
    unsafe extern "C" {
        /// Look up an Objective-C class by C string.
        /// Arguments: name is the class name. Returns: class pointer or null.
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        /// Look up an Objective-C selector by C string.
        /// Arguments: name is the selector name. Returns: selector pointer.
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        /// Send a state-setting Objective-C message.
        /// Arguments: receiver is a menu item; selector names the method; state is its check state. Returns: nothing.
        fn objc_msg_send_state(receiver: *mut c_void, selector: *mut c_void, state: isize);
        #[link_name = "objc_msgSend"]
        /// Send an Objective-C message with one object argument.
        /// Arguments: receiver and selector identify the call; object is its argument. Returns: returned object pointer.
        fn objc_msg_send_object(
            receiver: *mut c_void,
            selector: *mut c_void,
            object: *mut c_void,
        ) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        /// Send a setter message with an object argument.
        /// Arguments: receiver and selector identify the call; object is the new value. Returns: nothing.
        fn objc_msg_send_set_object(
            receiver: *mut c_void,
            selector: *mut c_void,
            object: *mut c_void,
        );
    }
    unsafe {
        let image_class = objc_getClass(c"NSImage".as_ptr());
        let image = objc_msg_send_object(
            image_class,
            sel_registerName(c"imageNamed:".as_ptr()),
            NSImageNameMenuOnStateTemplate,
        );
        // minifb replaces the standard on-state image with its empty off-state
        // image, so restore Cocoa's checkmark before setting the item state.
        objc_msg_send_set_object(
            item.0 as *mut c_void,
            sel_registerName(c"setOnStateImage:".as_ptr()),
            image,
        );
        objc_msg_send_state(
            item.0 as *mut c_void,
            sel_registerName(c"setState:".as_ptr()),
            isize::from(checked),
        );
    }
}

/// Other window backends do not expose a checkmark setter here.
#[cfg(not(target_os = "macos"))]
/// Accept a checkmark change on backends without a native state API.
/// Arguments: item is the menu handle; checked is the desired state. Returns: nothing.
fn set_menu_checked(_item: MenuItemHandle, _checked: bool) {}

/// The menu handles permit clean replacement when the display size changes.
struct WindowMenus {
    #[cfg(target_os = "macos")]
    top_items: [*mut std::ffi::c_void; 4],
    #[cfg(not(target_os = "macos"))]
    handles: [MenuHandle; 4],
    fast_mode_item: MenuItemHandle,
    sound_item: MenuItemHandle,
    joystick_help_item: MenuItemHandle,
    palette_items: [MenuItemHandle; 4],
}

impl WindowMenus {
    /// Remove this window's top-level menus before changing the display size.
    /// Arguments: self holds menu handles; window is the old window. Returns: nothing.
    fn remove(self, window: &mut Window) {
        #[cfg(target_os = "macos")]
        {
            let _ = window;
            macos_menu::remove(self.top_items);
        }
        #[cfg(not(target_os = "macos"))]
        for handle in self.handles.into_iter().rev() {
            window.remove_menu(handle);
        }
    }
}

/// Build the window and its menus for the current desktop settings.
/// Arguments: settings controls the display. Returns: window and menu handles or an error.
fn create_window(settings: DesktopSettings) -> Result<(Window, WindowMenus), String> {
    let (width, height) = frame_dimensions(settings);
    let mut window = Window::new(
        "XC16EMU Rust",
        width,
        height,
        WindowOptions {
            // Zoom is applied only to the emulated video. The status font and
            // its one-pixel outline remain the same size at every zoom level.
            scale: Scale::X1,
            resize: false,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| e.to_string())?;
    // The CPU has its own clock thread. The UI only needs display-rate refresh.
    window.set_target_fps(60);
    let menus = build_menus(&mut window, settings)?;
    #[cfg(target_os = "macos")]
    macos_menu::activate();
    Ok((window, menus))
}

/// Compute the exact host framebuffer size for one desktop configuration.
/// Arguments: settings supplies border, zoom, and status visibility. Returns:
/// width and height including the unscaled toolbar and optional status line.
fn frame_dimensions(settings: DesktopSettings) -> (usize, usize) {
    let (video_width, video_height) = settings.border.dimensions();
    let width = video_width * settings.zoom as usize;
    let height = TOOLBAR_HEIGHT
        + video_height * settings.zoom as usize
        + usize::from(settings.show_status) * STATUS_HEIGHT;
    (width, height)
}

/// On macOS, attach Machine and View without minifb's `add_menu`, which
/// incorrectly designates each one as AppKit's Window menu.
#[cfg(target_os = "macos")]
/// Populate Machine, View, and Window menus from saved choices.
/// Arguments: window is the host window; settings provides current check states. Returns: menu handles or an error.
fn build_menus(_window: &mut Window, settings: DesktopSettings) -> Result<WindowMenus, String> {
    let DesktopSettings {
        zoom,
        border,
        show_status,
        show_joystick_help,
        fast_mode,
        sound_enabled,
        palette,
        ..
    } = settings;
    let machine = macos_menu::new("Machine")?;
    macos_menu::item(machine, "Run/Stop", RUN_STOP_ID)?;
    macos_menu::item(machine, "Reset", RESET_ID)?;
    macos_menu::separator(machine);
    let fast_mode_item = macos_menu::item(machine, "Fast Mode", FAST_MODE_ID)?;
    set_menu_checked(fast_mode_item, fast_mode);
    macos_menu::separator(machine);
    let sound_item = macos_menu::item(machine, "Sound", SOUND_ID)?;
    set_menu_checked(sound_item, sound_enabled);
    let machine_top = macos_menu::attach(machine)?;

    let edit = macos_menu::new("Edit")?;
    macos_menu::paste_item(edit)?;
    macos_menu::item(edit, "Delete Program Line", DELETE_LINE_ID)?;
    let edit_top = macos_menu::attach(edit)?;

    let view = macos_menu::new("View")?;
    let status_item = macos_menu::item(view, "Show Status Bar", STATUS_ID)?;
    set_menu_checked(status_item, show_status);
    let joystick_help_item = macos_menu::item(view, "Show Joystick Keys", JOYSTICK_HELP_ID)?;
    set_menu_checked(joystick_help_item, show_joystick_help);
    macos_menu::item(view, "Show 128 Glyphs", GLYPH_TABLE_ID)?;
    macos_menu::separator(view);
    for (choice, label, id) in [
        (BorderSize::Big, "Border: Big", BIG_BORDER_ID),
        (BorderSize::Medium, "Border: Medium", MEDIUM_BORDER_ID),
        (BorderSize::Compact, "Border: Compact", COMPACT_BORDER_ID),
    ] {
        let item = macos_menu::item(view, label, id)?;
        set_menu_checked(item, border == choice);
    }
    macos_menu::separator(view);
    for (choice, id) in [(1, ZOOM_1_ID), (2, ZOOM_2_ID), (4, ZOOM_4_ID)] {
        let item = macos_menu::item(view, &format!("Zoom: {choice}×"), id)?;
        set_menu_checked(item, zoom == choice);
    }
    macos_menu::separator(view);
    let mut palette_items = Vec::with_capacity(PALETTE_CHOICES.len());
    for (choice, label, id) in PALETTE_CHOICES {
        let item = macos_menu::item(view, label, id)?;
        set_menu_checked(item, palette == choice);
        palette_items.push(item);
    }
    let palette_items = palette_items
        .try_into()
        .map_err(|_| "palette menu size mismatch")?;
    let view_top = macos_menu::attach(view)?;

    let window_menu = macos_menu::new("Window")?;
    let window_top = macos_menu::attach(window_menu)?;
    macos_menu::set_window_menu(window_menu);
    macos_menu::debug_check_layout(machine, edit, view, window_menu);
    Ok(WindowMenus {
        top_items: [machine_top, edit_top, view_top, window_top],
        fast_mode_item,
        sound_item,
        joystick_help_item,
        palette_items,
    })
}

/// Other minifb backends can use its ordinary menu API.
#[cfg(not(target_os = "macos"))]
/// Populate Machine, View, and Window menus from saved choices.
/// Arguments: window is the host window; settings provides current check states. Returns: menu handles or an error.
fn build_menus(window: &mut Window, settings: DesktopSettings) -> Result<WindowMenus, String> {
    let DesktopSettings {
        zoom,
        border,
        show_status,
        show_joystick_help,
        fast_mode,
        sound_enabled,
        palette,
        ..
    } = settings;
    let mut machine_menu = Menu::new("Machine").map_err(|e| e.to_string())?;
    machine_menu.add_item("Run/Stop", RUN_STOP_ID).build();
    machine_menu.add_item("Reset", RESET_ID).build();
    machine_menu.add_separator();
    let fast_mode_item = machine_menu.add_item("Fast Mode", FAST_MODE_ID).build();
    set_menu_checked(fast_mode_item, fast_mode);
    machine_menu.add_separator();
    let sound_item = machine_menu.add_item("Sound", SOUND_ID).build();
    set_menu_checked(sound_item, sound_enabled);
    let machine_handle = window.add_menu(&machine_menu);

    let mut edit_menu = Menu::new("Edit").map_err(|e| e.to_string())?;
    edit_menu
        .add_item("Paste", PASTE_ID)
        .shortcut(Key::V, minifb::MENU_KEY_CTRL)
        .build();
    edit_menu
        .add_item("Delete Program Line", DELETE_LINE_ID)
        .build();
    let edit_handle = window.add_menu(&edit_menu);

    let mut view_menu = Menu::new("View").map_err(|e| e.to_string())?;
    let status_item = view_menu.add_item("Show Status Bar", STATUS_ID).build();
    set_menu_checked(status_item, show_status);
    let joystick_help_item = view_menu
        .add_item("Show Joystick Keys", JOYSTICK_HELP_ID)
        .build();
    set_menu_checked(joystick_help_item, show_joystick_help);
    view_menu
        .add_item("Show 128 Glyphs", GLYPH_TABLE_ID)
        .build();
    view_menu.add_separator();
    for (choice, label, id) in [
        (BorderSize::Big, "Border: Big", BIG_BORDER_ID),
        (BorderSize::Medium, "Border: Medium", MEDIUM_BORDER_ID),
        (BorderSize::Compact, "Border: Compact", COMPACT_BORDER_ID),
    ] {
        let item = view_menu.add_item(label, id).build();
        set_menu_checked(item, border == choice);
    }
    view_menu.add_separator();
    for (choice, id) in [(1, ZOOM_1_ID), (2, ZOOM_2_ID), (4, ZOOM_4_ID)] {
        let item = view_menu.add_item(&format!("Zoom: {choice}×"), id).build();
        set_menu_checked(item, zoom == choice);
    }
    view_menu.add_separator();
    let mut palette_items = Vec::with_capacity(PALETTE_CHOICES.len());
    for (choice, label, id) in PALETTE_CHOICES {
        let item = view_menu.add_item(label, id).build();
        set_menu_checked(item, palette == choice);
        palette_items.push(item);
    }
    let palette_items = palette_items
        .try_into()
        .map_err(|_| "palette menu size mismatch")?;
    let view_handle = window.add_menu(&view_menu);

    let window_menu = Menu::new("Window").map_err(|e| e.to_string())?;
    let window_handle = window.add_menu(&window_menu);
    Ok(WindowMenus {
        handles: [machine_handle, edit_handle, view_handle, window_handle],
        fast_mode_item,
        sound_item,
        joystick_help_item,
        palette_items,
    })
}

/// Attach macOS menus without minifb's top-level attachment function. That
/// function calls `setWindowsMenu:` for every menu, which causes AppKit to
/// inject the system Window commands into Machine and View.
#[cfg(target_os = "macos")]
mod macos_menu {
    use super::MenuItemHandle;
    use std::ffi::{c_char, c_void, CStr, CString};

    type Object = *mut c_void;

    unsafe extern "C" {
        /// Create a minifb native menu.
        /// Arguments: name is its C-string title. Returns: native menu pointer.
        fn mfb_create_menu(name: *const c_char) -> Object;
        /// Append a minifb menu item or separator.
        /// Arguments: menu is the destination; id and name identify it; enabled, key, and modifier set behavior. Returns: the native item handle.
        fn mfb_add_menu_item(
            menu: Object,
            id: i32,
            name: *const c_char,
            enabled: bool,
            key: u32,
            modifier: u32,
        ) -> u64;
    }

    #[allow(clashing_extern_declarations)]
    #[link(name = "objc")]
    unsafe extern "C" {
        /// Look up an Objective-C class by C string.
        /// Arguments: name is the class name. Returns: class pointer or null.
        fn objc_getClass(name: *const c_char) -> Object;
        /// Look up an Objective-C selector by C string.
        /// Arguments: name is the selector name. Returns: selector pointer.
        fn sel_registerName(name: *const c_char) -> Object;
        #[link_name = "objc_msgSend"]
        /// Send a zero-argument Objective-C message.
        /// Arguments: receiver and selector identify the call. Returns: returned object pointer.
        fn message_0(receiver: Object, selector: Object) -> Object;
        #[link_name = "objc_msgSend"]
        /// Ask an Objective-C collection for its item count.
        /// Arguments: receiver and selector identify the call. Returns: number of items.
        fn message_count(receiver: Object, selector: Object) -> isize;
        #[link_name = "objc_msgSend"]
        /// Send an Objective-C message with a C-string argument.
        /// Arguments: receiver and selector identify the call; value is the string. Returns: returned object pointer.
        fn message_cstr(receiver: Object, selector: Object, value: *const c_char) -> Object;
        #[link_name = "objc_msgSend"]
        /// Send an Objective-C message with one object argument.
        /// Arguments: receiver and selector identify the call; value is its argument. Returns: nothing.
        fn message_1(receiver: Object, selector: Object, value: Object);
        #[link_name = "objc_msgSend"]
        /// Send an Objective-C message with one object argument and an object result.
        /// Arguments: receiver and selector identify the call; value is its argument. Returns: an object pointer.
        fn message_object_1(receiver: Object, selector: Object, value: Object) -> Object;
        #[link_name = "objc_msgSend"]
        /// Send an Objective-C message with an integer boolean.
        /// Arguments: receiver and selector identify the call; value is the boolean. Returns: nothing.
        fn message_bool(receiver: Object, selector: Object, value: isize);
        #[link_name = "objc_msgSend"]
        /// Send an Objective-C message with three object arguments.
        /// Arguments: receiver and selector identify the call; first, second, third are arguments. Returns: returned object pointer.
        fn message_3(
            receiver: Object,
            selector: Object,
            first: Object,
            second: Object,
            third: Object,
        ) -> Object;
    }

    /// Convert Rust text to an autoreleased NSString.
    /// Arguments: text is UTF-8 source text. Returns: object pointer or a NUL error.
    fn string(text: &str) -> Result<Object, String> {
        let text = CString::new(text).map_err(|_| "menu title contains NUL".to_string())?;
        unsafe {
            Ok(message_cstr(
                objc_getClass(c"NSString".as_ptr()),
                sel_registerName(c"stringWithUTF8String:".as_ptr()),
                text.as_ptr(),
            ))
        }
    }

    /// Get the shared NSApplication object.
    /// Arguments: none. Returns: application pointer.
    fn application() -> Object {
        unsafe {
            message_0(
                objc_getClass(c"NSApplication".as_ptr()),
                sel_registerName(c"sharedApplication".as_ptr()),
            )
        }
    }

    /// Get the active macOS menu bar.
    /// Arguments: none. Returns: menu pointer.
    fn main_menu() -> Object {
        unsafe { message_0(application(), sel_registerName(c"mainMenu".as_ptr())) }
    }

    /// Create a named native menu.
    /// Arguments: title is the menu label. Returns: menu pointer or an error.
    pub fn new(title: &str) -> Result<Object, String> {
        let title = CString::new(title).map_err(|_| "menu title contains NUL".to_string())?;
        let menu = unsafe { mfb_create_menu(title.as_ptr()) };
        if menu.is_null() {
            Err("could not create macOS menu".to_string())
        } else {
            Ok(menu)
        }
    }

    /// minifb's item builder supplies its `onMenuPress:` action, so the
    /// existing `Window::is_menu_pressed` still receives these IDs.
    /// Arguments: menu is the destination; label is visible text; id is the command identifier. Returns: item handle or an error.
    pub fn item(menu: Object, label: &str, id: usize) -> Result<MenuItemHandle, String> {
        let label = CString::new(label).map_err(|_| "menu item contains NUL".to_string())?;
        let handle = unsafe { mfb_add_menu_item(menu, id as i32, label.as_ptr(), true, 0x7f, 0) };
        if handle == 0 {
            Err("could not create macOS menu item".to_string())
        } else {
            Ok(MenuItemHandle(handle))
        }
    }

    /// Add Paste with AppKit's Command-V key equivalent.
    /// Arguments: menu is the Edit menu. Returns: a menu item or an error.
    pub fn paste_item(menu: Object) -> Result<MenuItemHandle, String> {
        let handle = unsafe {
            mfb_add_menu_item(
                menu,
                super::PASTE_ID as i32,
                c"Paste".as_ptr(),
                true,
                0x09,
                8,
            )
        };
        if handle == 0 {
            Err("could not create Paste menu item".to_string())
        } else {
            Ok(MenuItemHandle(handle))
        }
    }

    /// Read plain UTF-8 text from the macOS general pasteboard.
    /// Arguments: none. Returns: available text, no text, or an API error.
    pub fn clipboard_text() -> Result<Option<String>, String> {
        let text_type = string("public.utf8-plain-text")?;
        unsafe {
            let board = message_0(
                objc_getClass(c"NSPasteboard".as_ptr()),
                sel_registerName(c"generalPasteboard".as_ptr()),
            );
            if board.is_null() {
                return Err("macOS pasteboard is unavailable".to_string());
            }
            let value = message_object_1(
                board,
                sel_registerName(c"stringForType:".as_ptr()),
                text_type,
            );
            if value.is_null() {
                return Ok(None);
            }
            let bytes = message_0(value, sel_registerName(c"UTF8String".as_ptr())) as *const c_char;
            if bytes.is_null() {
                return Ok(None);
            }
            Ok(Some(CStr::from_ptr(bytes).to_string_lossy().into_owned()))
        }
    }

    /// Add a visual separator to a native menu.
    /// Arguments: menu is the destination. Returns: nothing.
    pub fn separator(menu: Object) {
        unsafe {
            mfb_add_menu_item(menu, -1, c"".as_ptr(), true, 0x7f, 0);
        }
    }

    /// Add a top-level menu without changing `NSApp.windowsMenu`.
    /// Arguments: menu is the native submenu. Returns: top-level item pointer or an error.
    pub fn attach(menu: Object) -> Result<Object, String> {
        let empty = string("")?;
        unsafe {
            let title = message_0(menu, sel_registerName(c"title".as_ptr()));
            if title.is_null() {
                return Err("macOS menu has no title".to_string());
            }
            let top = message_3(
                main_menu(),
                sel_registerName(c"addItemWithTitle:action:keyEquivalent:".as_ptr()),
                title,
                std::ptr::null_mut(),
                empty,
            );
            if top.is_null() {
                return Err("could not attach macOS menu".to_string());
            }
            message_1(top, sel_registerName(c"setSubmenu:".as_ptr()), menu);
            message_1(top, sel_registerName(c"setTitle:".as_ptr()), title);
            if message_count(
                message_0(top, sel_registerName(c"title".as_ptr())),
                sel_registerName(c"length".as_ptr()),
            ) <= 0
            {
                return Err("macOS menu bar item has an empty title".to_string());
            }
            message_bool(menu, sel_registerName(c"setAutoenablesItems:".as_ptr()), 0);
            Ok(top)
        }
    }

    /// Register the actual native Window menu with AppKit.
    /// Arguments: menu is the Window menu pointer or null. Returns: nothing.
    pub fn set_window_menu(menu: Object) {
        unsafe {
            message_1(
                application(),
                sel_registerName(c"setWindowsMenu:".as_ptr()),
                menu,
            );
        }
    }

    /// Activate the app synchronously after its menus have been attached.
    /// Arguments: none. Returns: nothing; AppKit shows the menu bar at launch.
    pub fn activate() {
        unsafe {
            // minifb activates through an asynchronous main-queue callback.
            // Its manually pumped event loop need not run that callback before
            // the user first reaches the menu bar.
            message_bool(
                application(),
                sel_registerName(c"activateIgnoringOtherApps:".as_ptr()),
                1,
            );
            message_bool(
                objc_getClass(c"NSMenu".as_ptr()),
                sel_registerName(c"setMenuBarVisible:".as_ptr()),
                1,
            );
        }
    }

    /// Bring an existing modeless dialog forward when its toolbar button is used.
    /// Arguments: window is the native NSWindow pointer. Returns: nothing.
    pub fn focus_window(window: Object) {
        unsafe {
            message_1(
                window,
                sel_registerName(c"makeKeyAndOrderFront:".as_ptr()),
                std::ptr::null_mut(),
            );
        }
    }

    /// Catch menu setup regressions during desktop development. System Window
    /// commands may grow later, but must never land in Machine or View.
    /// Arguments: machine, edit, view, and window are native menu pointers. Returns: nothing; debug assertions may fail.
    pub fn debug_check_layout(machine: Object, edit: Object, view: Object, window: Object) {
        #[cfg(debug_assertions)]
        unsafe {
            let count = sel_registerName(c"numberOfItems".as_ptr());
            debug_assert_eq!(message_count(machine, count), 5);
            debug_assert_eq!(message_count(edit, count), 2);
            debug_assert_eq!(message_count(view, count), 16);
            debug_assert_eq!(
                message_0(application(), sel_registerName(c"windowsMenu".as_ptr())),
                window
            );
        }
        #[cfg(not(debug_assertions))]
        let _ = (machine, edit, view, window);
    }

    /// Remove the four native top-level menu items and clear AppKit's Window menu.
    /// Arguments: top_items contains Machine, Edit, View, and Window item pointers. Returns: nothing.
    pub fn remove(top_items: [Object; 4]) {
        set_window_menu(std::ptr::null_mut());
        unsafe {
            for item in top_items.into_iter().rev() {
                message_1(main_menu(), sel_registerName(c"removeItem:".as_ptr()), item);
            }
        }
    }
}

/// Read plain clipboard text using the platform's clipboard command.
/// Arguments: none. Returns: available text or a clear command failure.
#[cfg(not(target_os = "macos"))]
fn clipboard_text() -> Result<Option<String>, String> {
    use std::process::Command;
    for (program, args) in [
        ("wl-paste", &["--no-newline"][..]),
        ("xclip", &["-selection", "clipboard", "-out"][..]),
        ("xsel", &["--clipboard", "--output"][..]),
    ] {
        if let Ok(output) = Command::new(program).args(args).output() {
            if output.status.success() {
                return String::from_utf8(output.stdout)
                    .map(Some)
                    .map_err(|error| error.to_string());
            }
        }
    }
    Err("no supported clipboard command found".to_string())
}

/// Read plain text from the native macOS pasteboard.
/// Arguments: none. Returns: available text, no text, or a pasteboard error.
#[cfg(target_os = "macos")]
fn clipboard_text() -> Result<Option<String>, String> {
    macos_menu::clipboard_text()
}

/// Five-by-seven glyphs shared by the status meter and joystick help panel.
/// Arguments: character is the requested glyph. Returns: seven bitmap rows, or blank for unsupported characters.
fn status_glyph(character: char) -> [u8; 7] {
    match character {
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
        '.' => [0, 0, 0, 0, 0, 0b00100, 0b00100],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '%' => [0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b00110, 0],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        _ => [0; 7],
    }
}

/// Draw a short status string directly into the RGB window buffer.
/// Arguments: buffer is the output; width is row stride; x/y are origin; text is content; color is RGB. Returns: nothing.
fn draw_status_text(buffer: &mut [u32], width: usize, x: usize, y: usize, text: &str, color: u32) {
    for (index, character) in text.chars().enumerate() {
        for (dy, row) in status_glyph(character).iter().enumerate() {
            for dx in 0..5 {
                if row & (1 << (4 - dx)) != 0 {
                    buffer[(y + dy) * width + x + index * 6 + dx] = color;
                }
            }
        }
    }
}

/// Fill a rectangular UI area without changing the emulator's video pixels.
/// Arguments: buffer is a packed RGB frame; width is its stride; x/y and w/h
/// locate the rectangle; color is packed RGB. Returns: nothing.
fn fill_ui_rect(
    buffer: &mut [u32],
    width: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    for row in y..y + h {
        buffer[row * width + x..row * width + x + w].fill(color);
    }
}

/// One of the fixed toolbar buttons above the emulated video.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolbarAction {
    Fast,
    Sound,
    Colors,
}

/// Resolve a left-click in the unscaled toolbar to one command.
/// Arguments: position is a window-relative mouse coordinate. Returns: the
/// clicked button, or None outside all three controls.
fn toolbar_action(position: (f32, f32)) -> Option<ToolbarAction> {
    if !(5.0..23.0).contains(&position.1) {
        return None;
    }
    match position.0 {
        x if (8.0..84.0).contains(&x) => Some(ToolbarAction::Fast),
        x if (92.0..168.0).contains(&x) => Some(ToolbarAction::Sound),
        x if (176.0..260.0).contains(&x) => Some(ToolbarAction::Colors),
        _ => None,
    }
}

/// Draw a compact host toolbar whose controls stay one pixel per UI pixel.
/// Arguments: buffer is the window frame; width is its stride; settings set
/// the button states; colors_open marks the dialog button. Returns: nothing.
fn draw_toolbar(buffer: &mut [u32], width: usize, settings: DesktopSettings, colors_open: bool) {
    fill_ui_rect(buffer, width, 0, 0, width, TOOLBAR_HEIGHT, 0x1b2028);
    fill_ui_rect(buffer, width, 0, TOOLBAR_HEIGHT - 1, width, 1, 0x59616c);
    for (x, w, label, active) in [
        (8, 76, "FAST", settings.fast_mode),
        (92, 76, "SOUND", settings.sound_enabled),
        (176, 84, "COLORS", colors_open),
    ] {
        fill_ui_rect(
            buffer,
            width,
            x,
            5,
            w,
            18,
            if active { 0x294e70 } else { 0x303843 },
        );
        fill_ui_rect(
            buffer,
            width,
            x,
            5,
            w,
            1,
            if active { 0x69a9dc } else { 0x69727e },
        );
        fill_ui_rect(buffer, width, x, 22, w, 1, 0x10151a);
        let label_x = x + (w - label.len() * 6 + 1) / 2;
        draw_status_text(
            buffer,
            width,
            label_x,
            10,
            label,
            if active { 0xffffff } else { 0xcbd3dc },
        );
    }
}

/// Identify the slider currently held by the mouse in the Colors window.
#[derive(Clone, Copy)]
enum ColorSlider {
    Brightness,
    Contrast,
}

/// A separate non-modal host window for live palette adjustments.
struct ColorsDialog {
    window: Window,
    dragging: Option<ColorSlider>,
    frame: Vec<u32>,
}

impl ColorsDialog {
    /// Create and position the dialog near the emulator without blocking it.
    /// Arguments: main is the simulator window. Returns: the dialog or an error.
    fn new(main: &Window) -> Result<Self, String> {
        let mut window = Window::new(
            "Colors",
            COLOR_DIALOG_WIDTH,
            COLOR_DIALOG_HEIGHT,
            WindowOptions {
                scale: Scale::X1,
                resize: false,
                ..WindowOptions::default()
            },
        )
        .map_err(|e| e.to_string())?;
        let (x, y) = main.get_position();
        window.set_position(x + 40, y + 40);
        window.set_target_fps(60);
        Ok(Self {
            window,
            dragging: None,
            // minifb's macOS MTKView retains this pointer until its next
            // draw, which can happen after update_with_buffer returns.
            frame: vec![0; COLOR_DIALOG_WIDTH * COLOR_DIALOG_HEIGHT],
        })
    }

    /// Process one mouse step and redraw the controls and 121-color preview.
    /// Arguments: self owns the dialog; settings receives live adjustments.
    /// Returns: None if closed, otherwise whether values changed or a drag ended.
    fn update(&mut self, settings: &mut DesktopSettings) -> Result<Option<(bool, bool)>, String> {
        if !self.window.is_open() || self.window.is_key_down(Key::Escape) {
            return Ok(None);
        }
        let down = self.window.get_mouse_down(MouseButton::Left);
        let position = self.window.get_mouse_pos(MouseMode::Pass);
        let was_dragging = self.dragging.is_some();
        if down && self.dragging.is_none() {
            if let Some((_, y)) = position {
                self.dragging = if (56.0..82.0).contains(&y) {
                    Some(ColorSlider::Brightness)
                } else if (105.0..131.0).contains(&y) {
                    Some(ColorSlider::Contrast)
                } else {
                    None
                };
            }
        }
        let mut changed = false;
        if down {
            if let (Some(slider), Some((x, _))) = (self.dragging, position) {
                let t = ((x - SLIDER_START as f32) / (SLIDER_END - SLIDER_START) as f32)
                    .clamp(0.0, 1.0);
                match slider {
                    ColorSlider::Brightness => {
                        let value = slider_value(t, -100, 100) as i16;
                        changed = settings.brightness != value;
                        settings.brightness = value;
                    }
                    ColorSlider::Contrast => {
                        let value = slider_value(t, 50, 200) as u16;
                        changed = settings.contrast != value;
                        settings.contrast = value;
                    }
                }
            }
        } else {
            self.dragging = None;
        }
        render_colors_dialog(*settings, &mut self.frame);
        self.window
            .update_with_buffer(&self.frame, COLOR_DIALOG_WIDTH, COLOR_DIALOG_HEIGHT)
            .map_err(|e| e.to_string())?;
        Ok(Some((changed, was_dragging && !down)))
    }
}

/// Convert a clamped slider position to a whole-number percentage.
/// Arguments: t is normalized 0..1 position; min/max bound the control.
/// Returns: the nearest integer setting within the control range.
fn slider_value(t: f32, min: i32, max: i32) -> i32 {
    min + (t.clamp(0.0, 1.0) * (max - min) as f32).round() as i32
}

/// Render the slider window and one swatch for each distinct TED color.
/// Arguments: settings supplies the palette and live adjustments; frame is
/// the persistent 360x260 RGB buffer. Returns: nothing; the buffer is filled.
fn render_colors_dialog(settings: DesktopSettings, mut frame: &mut [u32]) {
    frame.fill(0x171d25);
    draw_status_text(
        &mut frame,
        COLOR_DIALOG_WIDTH,
        20,
        14,
        "121 COLORS",
        0xe5ebf2,
    );
    draw_color_slider(
        &mut frame,
        "BRIGHTNESS",
        40,
        settings.brightness as i32,
        -100,
        100,
    );
    draw_color_slider(
        &mut frame,
        "CONTRAST",
        89,
        settings.contrast as i32,
        50,
        200,
    );
    draw_status_text(
        &mut frame,
        COLOR_DIALOG_WIDTH,
        20,
        137,
        "PALETTE PREVIEW",
        0xaebbc9,
    );
    for row in 0..8 {
        for hue in 1..16 {
            let rgb = adjust_rgb(
                settings.palette.rgb((row * 16 + hue) as u8),
                settings.brightness,
                settings.contrast,
            );
            fill_ui_rect(
                &mut frame,
                COLOR_DIALOG_WIDTH,
                20 + (hue - 1) * 20,
                153 + row * 11,
                19,
                10,
                rgb,
            );
        }
    }
    let black = adjust_rgb(
        settings.palette.rgb(0),
        settings.brightness,
        settings.contrast,
    );
    fill_ui_rect(&mut frame, COLOR_DIALOG_WIDTH, 320, 153, 20, 20, black);
    draw_status_text(&mut frame, COLOR_DIALOG_WIDTH, 315, 181, "BLACK", 0xaebbc9);
    draw_status_text(
        &mut frame,
        COLOR_DIALOG_WIDTH,
        20,
        247,
        "CHANGES APPLY LIVE",
        0x8d9aaa,
    );
}

/// Draw one labeled slider with its numeric percentage.
/// Arguments: frame is the dialog RGB buffer; label names the control; y is
/// the label row; value lies between min and max. Returns: nothing.
fn draw_color_slider(frame: &mut [u32], label: &str, y: usize, value: i32, min: i32, max: i32) {
    draw_status_text(frame, COLOR_DIALOG_WIDTH, 20, y, label, 0xcbd5df);
    let value_text = if min < 0 {
        format!("{value:+}%")
    } else {
        format!("{value}%")
    };
    draw_status_text(frame, COLOR_DIALOG_WIDTH, 275, y, &value_text, 0xffffff);
    fill_ui_rect(
        frame,
        COLOR_DIALOG_WIDTH,
        SLIDER_START,
        y + 23,
        SLIDER_END - SLIDER_START,
        4,
        0x465361,
    );
    let x = SLIDER_START
        + ((value - min) as usize * (SLIDER_END - SLIDER_START) / (max - min) as usize);
    fill_ui_rect(
        frame,
        COLOR_DIALOG_WIDTH,
        x.saturating_sub(4),
        y + 17,
        9,
        16,
        0x83c7f3,
    );
}

/// Draw the same pixel font at twice its native size for readable help text.
/// Arguments: buffer is the output; width is row stride; x/y are origin; text is content; color is RGB. Returns: nothing.
fn draw_help_text(buffer: &mut [u32], width: usize, x: usize, y: usize, text: &str, color: u32) {
    for (index, character) in text.chars().enumerate() {
        for (dy, row) in status_glyph(character).iter().enumerate() {
            for dx in 0..5 {
                if row & (1 << (4 - dx)) != 0 {
                    for scale_y in 0..2 {
                        let start = (y + dy * 2 + scale_y) * width + x + index * 12 + dx * 2;
                        buffer[start..start + 2].fill(color);
                    }
                }
            }
        }
    }
}

/// Opaque in-video key guide. It is redrawn after each emulated frame and
/// never changes the machine's pixels or the unscaled status strip.
/// Arguments: buffer is the output; width is row stride; video_height excludes status. Returns: nothing.
fn draw_joystick_help(buffer: &mut [u32], width: usize, video_height: usize) {
    const PANEL_WIDTH: usize = 280;
    const PANEL_HEIGHT: usize = 190;
    let x = (width - PANEL_WIDTH) / 2;
    let y = TOOLBAR_HEIGHT + (video_height - PANEL_HEIGHT) / 2;
    for row in y..y + PANEL_HEIGHT {
        let pixels = &mut buffer[row * width + x..row * width + x + PANEL_WIDTH];
        pixels.fill(if row == y || row == y + PANEL_HEIGHT - 1 {
            0x77818b
        } else {
            0x101820
        });
        pixels[0] = 0x77818b;
        pixels[PANEL_WIDTH - 1] = 0x77818b;
    }
    let text_x = x + 16;
    for (offset, label, color) in [
        (10, "JOYSTICK KEYS", 0xffffff),
        (32, "PORT 1", 0x7fc5ff),
        (48, "UP 5   DOWN R", 0xffffff),
        (64, "LEFT D RIGHT 6", 0xffffff),
        (80, "FIRE T", 0xffffff),
        (104, "PORT 2", 0x7fc5ff),
        (120, "UP 3   DOWN W", 0xffffff),
        (136, "LEFT A RIGHT 4", 0xffffff),
        (152, "FIRE SHIFT", 0xffffff),
        (173, "USE MENU TO CLOSE", 0x9ba9b5),
    ] {
        draw_help_text(buffer, width, text_x, y + offset, label, color);
    }
}

/// Scale only the C16 picture; status pixels are composed afterward at 1×.
/// Arguments: buffer is the output; width is row stride; pixels are TED colors;
/// video_width and zoom set layout; palette and brightness/contrast map TED
/// colors to RGB. Returns: nothing; pixels start below the toolbar.
fn draw_video(
    buffer: &mut [u32],
    width: usize,
    pixels: &[u8],
    video_width: usize,
    zoom: usize,
    settings: DesktopSettings,
) {
    for (y, row) in pixels.chunks_exact(video_width).enumerate() {
        for (x, &color) in row.iter().enumerate() {
            let rgb = adjust_rgb(
                settings.palette.rgb(color),
                settings.brightness,
                settings.contrast,
            );
            for dy in 0..zoom {
                let start = (TOOLBAR_HEIGHT + y * zoom + dy) * width + x * zoom;
                buffer[start..start + zoom].fill(rgb);
            }
        }
    }
}

/// Locate the REALTIME label and value at the status font's native size.
/// Arguments: mips and realtime are the displayed meter values. Returns: the horizontal pixel range.
fn realtime_text_range(mips: f64, realtime: f64) -> std::ops::Range<usize> {
    let start = 10 + 6 * 6 + format!("{mips:.3}").len() * 6 + 18;
    let end = start + ("REALTIME: ".len() + format!("{realtime:.2}X").len()) * 6;
    start..end
}

/// Test a window-relative mouse position against the unscaled REALTIME text.
/// Arguments: position is the mouse location; video_height is the status top; meters set text width. Returns: true inside the text.
fn realtime_text_hit(position: (f32, f32), video_height: usize, mips: f64, realtime: f64) -> bool {
    let range = realtime_text_range(mips, realtime);
    position.0 >= range.start as f32
        && position.0 < range.end as f32
        && position.1 >= (video_height + 1) as f32
        && position.1 < (video_height + STATUS_HEIGHT - 1) as f32
}

/// A one-pixel gray outline separates the black status strip from the video.
/// Arguments: buffer is the output; width is row stride; video_height locates the strip; mips/realtime are meters; sound_enabled sets icon. Returns: nothing.
fn draw_status(
    buffer: &mut [u32],
    width: usize,
    video_height: usize,
    mips: f64,
    realtime: f64,
    sound_enabled: bool,
) {
    let outline = 0x555555;
    buffer[video_height * width..(video_height + 1) * width].fill(outline);
    buffer[(video_height + STATUS_HEIGHT - 1) * width..(video_height + STATUS_HEIGHT) * width]
        .fill(outline);
    for y in video_height + 1..video_height + STATUS_HEIGHT - 1 {
        buffer[y * width] = outline;
        buffer[y * width + width - 1] = outline;
    }
    let y = video_height + 2;
    // Keep text clear of the window's rounded lower-left corner.
    let label_x = 10;
    draw_status_text(buffer, width, label_x, y, "MIPS: ", 0x888888);
    let mips_value = format!("{mips:.3}");
    let mips_x = label_x + 6 * 6;
    draw_status_text(buffer, width, mips_x, y, &mips_value, 0xffffff);
    let realtime_x = realtime_text_range(mips, realtime).start;
    draw_status_text(buffer, width, realtime_x, y, "REALTIME: ", 0x888888);
    draw_status_text(
        buffer,
        width,
        realtime_x + 10 * 6,
        y,
        &format!("{realtime:.2}X"),
        0xffffff,
    );
    // A fixed-size speaker stays at the right edge at every video zoom.
    // White sound waves mean enabled; a gray slash means muted.
    let speaker = [
        0b00100, 0b01100, 0b11100, 0b11100, 0b11100, 0b01100, 0b00100,
    ];
    let waves = [
        0b0001000, 0b0010010, 0b0100001, 0b0100001, 0b0100001, 0b0010010, 0b0001000,
    ];
    let x = width - 20;
    for dy in 0..7 {
        for dx in 0..5 {
            if speaker[dy] & (1 << (4 - dx)) != 0 {
                buffer[(y + dy) * width + x + dx] = if sound_enabled { 0xffffff } else { 0x888888 };
            }
        }
        if sound_enabled {
            for dx in 0..7 {
                if waves[dy] & (1 << (6 - dx)) != 0 {
                    buffer[(y + dy) * width + x + 6 + dx] = 0xffffff;
                }
            }
        } else {
            buffer[(y + dy) * width + x + 7 + dy] = 0x888888;
        }
    }
}

/// Parse a decimal frame count from a command-line option.
/// Arguments: value is the optional argument string. Returns: frame count or an error.
fn parse_frames(value: Option<String>) -> Result<u64, String> {
    value
        .ok_or("--frames needs a number".to_string())?
        .parse()
        .map_err(|_| "invalid frame count".to_string())
}

/// Monitor addresses are hexadecimal, with or without a $ or 0x prefix.
/// Arguments: value is the optional address string. Returns: 16-bit address or an error.
fn parse_go_address(value: Option<String>) -> Result<u16, String> {
    let value = value.ok_or("--go needs a hexadecimal address")?;
    let digits = value
        .strip_prefix('$')
        .or_else(|| value.strip_prefix("0x"))
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(&value);
    u16::from_str_radix(digits, 16)
        .map_err(|_| format!("invalid --go address {value:?}; use hexadecimal, e.g. 4000"))
}

/// Translate a desktop key to the corresponding C16 matrix position.
/// Arguments: key is the host key code. Returns: matrix index or None when unmapped.
fn matrix_key(key: Key) -> Option<u8> {
    Some(match key {
        Key::Backspace => 0,
        Key::Delete | Key::F8 => 3,
        Key::Enter => 1,
        Key::F1 => 4,
        Key::F2 => 5,
        Key::F3 => 6,
        Key::Key3 => 8,
        Key::W => 9,
        Key::A => 10,
        Key::Key4 => 11,
        Key::Z => 12,
        Key::S => 13,
        Key::E => 14,
        Key::Key5 => 16,
        Key::R => 17,
        Key::D => 18,
        Key::Key6 => 19,
        Key::C => 20,
        Key::F => 21,
        Key::T => 22,
        Key::X => 23,
        Key::Key7 => 24,
        Key::Y => 25,
        Key::G => 26,
        Key::Key8 => 27,
        Key::B => 28,
        Key::H => 29,
        Key::U => 30,
        Key::V => 31,
        Key::Key9 => 32,
        Key::I => 33,
        Key::J => 34,
        Key::Key0 => 35,
        Key::M => 36,
        Key::K => 37,
        Key::O => 38,
        Key::N => 39,
        Key::Down => 40,
        Key::P => 41,
        Key::L => 42,
        Key::Up => 43,
        Key::Period => 44,
        Key::Semicolon => 45,
        Key::Minus => 46,
        Key::Comma => 47,
        Key::Left => 48,
        Key::Apostrophe => 49,
        Key::Right => 51,
        Key::Escape => 52,
        Key::Equal => 53,
        Key::Slash => 55,
        Key::Key1 => 56,
        Key::Home => 57,
        Key::LeftCtrl | Key::RightCtrl => 58,
        Key::Key2 => 59,
        Key::Space => 60,
        Key::LeftAlt | Key::RightAlt => 61,
        Key::Q => 62,
        Key::Pause | Key::F15 => 63,
        Key::Tab => 63,
        Key::LeftShift | Key::RightShift => 15,
        _ => return None,
    })
}

/// Release held host keys after one emulated frame in fast mode. Modifier
/// keys stay down so Shift and C= combinations can accompany a new key edge.
/// Arguments: machine receives releases and keys are currently held host keys.
/// Returns: nothing; later frames see no repeat until another host key press.
fn release_fast_mode_keys(machine: &mut Machine, keys: impl IntoIterator<Item = Key>) {
    for key in keys {
        if let Some(index) = matrix_key(key) {
            if !matches!(index, 15 | 61 | 63) {
                machine.set_key(index, false);
            }
        }
    }
}

/// Print the package version, copyright and project URL.
/// Arguments: none. Returns: nothing; writes one line to standard output.
fn print_version() {
    println!(
        "xc16emu version {} *** Copyright (c) 2026 Johannes Overmann *** https://github.com/jovermann/xc16emu",
        env!("CARGO_PKG_VERSION")
    );
}

/// Print the grouped command-line reference followed by version and copyright.
/// Arguments: none. Returns: nothing; writes help to standard output.
fn print_help() {
    print!(
        "{}",
        r#"xc16emu: C16/C116/Plus/4 emulator for macOS.

Usage: xc16emu [OPTIONS]

ROM and program options:
  --roms DIR              Read basrom.dat and sysrom.dat from DIR (default: old/roms).
  --ram 16|32|64           Set RAM size in KiB (default: 64).
  --prg FILE               Load a PRG after BASIC boots.
  --run-prg FILE           Load after boot and run at the PRG load address.
  --preload-prg FILE       Load a PRG before BASIC boots.
  --run-preload-prg FILE   Load before boot; run at its load address after boot.
  --go HEX                 Start at HEX after boot; overrides a --run address.
  --frozen-image FILE      Restore a 64 KiB frozen machine image.

Drive options:
  --drive DIR              Expose DIR as virtual device 8 (default: current directory).
  --dry-run                Accept drive saves without writing files.
  -v --verbose             Report drive file opens.

Execution and capture options:
  --type TEXT              Type a BASIC line after boot; may be repeated.
  --headless               Run without opening a window.
  --frames N               Run N frames in headless mode (default: 250).
  --text                   Print the 40x25 text screen; implies --headless.
  --graphics               Print the 320x200 graphics screen; implies --headless.

Desktop options:
  --zoom 1|2|4             Set the initial video zoom.
  --palette NAME           Use c16colors (default), c16colors-125, c16colors-150, or legacy.
  --speed FACTOR           Set the paced emulation speed (default: 1).
  --small-border           Use the compact border.

Common options:
  -h --help                Print this help message and exit.
     --version             Print version and exit.

"#
    );
    print_version();
}

/// Parse options, boot the ROM, and run in capture or desktop mode.
/// Arguments: CLI arguments and environment are read internally. Returns: success or a diagnostic error.
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut roms = PathBuf::from("old/roms");
    let mut frames = 250u64;
    let mut headless = false;
    let mut text = false;
    let mut graphics = false;
    let mut preload_prg = None;
    let mut prg = None;
    let mut go_address = None;
    let mut frozen_image = None;
    let mut typed = Vec::new();
    let mut ram = 64;
    let mut drive = PathBuf::from(".");
    let mut dry_run = false;
    let mut verbose = false;
    let mut zoom_override = None;
    let mut speed = 1.0f64;
    let mut border_override = None;
    let mut palette_override = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--roms" => roms = PathBuf::from(args.next().ok_or("--roms needs a path")?),
            "--drive" => drive = PathBuf::from(args.next().ok_or("--drive needs a path")?),
            "--dry-run" => dry_run = true,
            "--verbose" | "-v" => verbose = true,
            "--small-border" => border_override = Some(BorderSize::Compact),
            "--palette" => {
                let name = args
                    .next()
                    .ok_or("--palette needs a palette name (see --help)")?;
                palette_override = Some(Palette::parse(&name).ok_or_else(|| {
                    format!("unknown palette {name:?}; see --help for available palettes")
                })?);
            }
            "--zoom" => {
                let zoom = args
                    .next()
                    .ok_or("--zoom needs 1, 2, or 4")?
                    .parse()
                    .map_err(|_| "invalid zoom".to_string())?;
                if ![1, 2, 4].contains(&zoom) {
                    return Err("zoom must be 1, 2, or 4".to_string());
                }
                zoom_override = Some(zoom);
            }
            "--speed" => {
                speed = args
                    .next()
                    .ok_or("--speed needs a factor")?
                    .parse()
                    .map_err(|_| "invalid speed".to_string())?;
                if speed <= 0.0 {
                    return Err("speed must be positive".to_string());
                }
            }
            "--frames" => frames = parse_frames(args.next())?,
            "--headless" => headless = true,
            "--text" => {
                text = true;
                headless = true;
            }
            "--graphics" => {
                graphics = true;
                headless = true;
            }
            "--preload-prg" => {
                preload_prg = Some((
                    PathBuf::from(args.next().ok_or("--preload-prg needs a path")?),
                    false,
                ));
            }
            "--run-preload-prg" => {
                preload_prg = Some((
                    PathBuf::from(args.next().ok_or("--run-preload-prg needs a path")?),
                    true,
                ));
            }
            "--prg" => {
                prg = Some((
                    PathBuf::from(args.next().ok_or("--prg needs a path")?),
                    false,
                ));
            }
            "--run-prg" => {
                prg = Some((
                    PathBuf::from(args.next().ok_or("--run-prg needs a path")?),
                    true,
                ));
            }
            "--go" => go_address = Some(parse_go_address(args.next())?),
            "--frozen-image" => {
                frozen_image = Some(PathBuf::from(
                    args.next().ok_or("--frozen-image needs a path")?,
                ))
            }
            "--type" => typed.push(args.next().ok_or("--type needs BASIC text")?),
            "--ram" => {
                ram = args
                    .next()
                    .ok_or("--ram needs a value")?
                    .parse()
                    .map_err(|_| "invalid RAM size".to_string())?
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" => {
                print_version();
                return Ok(());
            }
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    let mut machine = Machine::from_rom_dir(&roms).map_err(|e| e.to_string())?;
    machine.set_ram_size(ram).map_err(str::to_string)?;
    machine.set_drive_root(drive);
    machine.set_dry_run(dry_run);
    machine.set_verbose(verbose);
    if let Some(path) = frozen_image {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        machine.load_frozen_image(&data).map_err(str::to_string)?;
    }
    let mut run_address = None;
    if let Some((path, run_loaded_prg)) = preload_prg {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        let address = machine.load_prg(&data).map_err(str::to_string)?;
        if run_loaded_prg {
            run_address = Some(address);
        }
    }
    machine.run_frames(100)?;
    let typed_count = typed.len();
    for (index, line) in typed.into_iter().enumerate() {
        machine.type_text(&line)?;
        machine.type_text("\n")?;
        if index + 1 < typed_count {
            // The Return key can start a long ROM command such as DLOAD or
            // GRAPHIC. Let it finish before typing the next startup line.
            machine.run_frames(50)?;
        }
    }
    if let Some((path, run_loaded_prg)) = prg {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        let address = machine.load_prg(&data).map_err(str::to_string)?;
        if run_loaded_prg {
            run_address = Some(address);
        }
    }
    let start_address = go_address.or(run_address);
    if let Some(address) = start_address {
        machine.cpu.pc = address;
    }
    if headless {
        machine.run_frames(frames)?;
        if text {
            println!("{}", machine.capture_text_screen());
        }
        if graphics {
            println!("{}", machine.capture_graphics_screen());
        }
        if !text && !graphics {
            println!("Ran {frames} PAL frames; PC=${:04X}", machine.cpu.pc);
        }
        return Ok(());
    }
    let config_path = settings_path()?;
    let mut settings = load_settings(&config_path);
    if let Some(zoom) = zoom_override {
        settings.zoom = zoom;
    }
    if let Some(border) = border_override {
        settings.border = border;
    }
    if let Some(palette) = palette_override {
        settings.palette = palette;
    }
    let audio = if settings.sound_enabled {
        match AudioOutput::open() {
            Ok(output) => {
                machine.set_audio_sample_rate(output.sample_rate());
                Some(output)
            }
            Err(error) => {
                eprintln!("sound unavailable: {error}");
                settings.sound_enabled = false;
                None
            }
        }
    } else {
        None
    };
    let mut charset_combo_was_down = false;
    // minifb keeps the supplied pointer for asynchronous Metal draws. Keep
    // this allocation alive and at a fixed address for the window's lifetime.
    let (frame_width, frame_height) = frame_dimensions(settings);
    let mut rgb = vec![0u32; frame_width * frame_height];
    let (mut window, mut menus) = create_window(settings)?;
    if !config_path.exists() {
        save_settings(&config_path, settings);
    }
    let mut meter_time = Instant::now();
    let mut meter_instructions = machine.instructions_executed;
    let mut meter_ticks = machine.ted.ticks;
    let mut mips = 0.0;
    let mut realtime = 0.0;
    let mut left_was_down = false;
    let audio_debug = env::var_os("XC16EMU_AUDIO_DEBUG").is_some();
    let machine = Arc::new(Mutex::new(machine));
    let audio = Arc::new(Mutex::new(audio));
    let input = Arc::new(WorkerInput::default());
    input.fast_mode.store(settings.fast_mode, Ordering::Relaxed);
    let worker = EmulatorWorker::start(
        Arc::clone(&machine),
        Arc::clone(&audio),
        Arc::clone(&input),
        speed,
    );
    let mut colors_dialog: Option<ColorsDialog> = None;
    let mut color_settings_dirty = false;
    #[cfg(target_os = "macos")]
    let mut activate_after_first_event = true;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        if let Some(error) = worker.error() {
            return Err(error);
        }
        // A setting that changes the video crop or status area changes the
        // fixed-size minifb buffer, so it requires a new window.
        let mut recreate_window = false;
        let mut persist_settings = false;
        let mut toggle_fast_mode = false;
        let mut toggle_sound = false;
        let mut open_colors = false;
        if let Some(id) = window.is_menu_pressed() {
            match id {
                RUN_STOP_ID => {
                    input.run_stop_frames.store(3, Ordering::Relaxed);
                    let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
                    input
                        .paste
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clear(&mut machine);
                }
                RESET_ID => {
                    let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
                    input
                        .paste
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clear(&mut machine);
                    machine.reset();
                    if let Some(output) = audio.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                        output.clear();
                    }
                    meter_ticks = machine.ted.ticks;
                    meter_instructions = machine.instructions_executed;
                    meter_time = Instant::now();
                }
                FAST_MODE_ID => toggle_fast_mode = true,
                PASTE_ID => match clipboard_text() {
                    Ok(Some(text)) => {
                        if let Err(error) = input
                            .paste
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .append(&text)
                        {
                            eprintln!("paste: {error}");
                        }
                    }
                    Ok(None) => {}
                    Err(error) => eprintln!("paste: {error}"),
                },
                DELETE_LINE_ID => {
                    let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
                    let mut paste = input.paste.lock().unwrap_or_else(|e| e.into_inner());
                    paste.clear(&mut machine);
                    paste.append_key(3); // Clear text, retaining the line number.
                    paste.append_key(1); // Submit that number to delete the record.
                }
                SOUND_ID => toggle_sound = true,
                STATUS_ID => {
                    settings.show_status = !settings.show_status;
                    recreate_window = true;
                    persist_settings = true;
                }
                JOYSTICK_HELP_ID => {
                    settings.show_joystick_help = !settings.show_joystick_help;
                    set_menu_checked(menus.joystick_help_item, settings.show_joystick_help);
                    persist_settings = true;
                }
                GLYPH_TABLE_ID => {
                    machine
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .write_glyph_table_to_screen();
                }
                BIG_BORDER_ID | MEDIUM_BORDER_ID | COMPACT_BORDER_ID => {
                    settings.border = match id {
                        BIG_BORDER_ID => BorderSize::Big,
                        MEDIUM_BORDER_ID => BorderSize::Medium,
                        _ => BorderSize::Compact,
                    };
                    recreate_window = true;
                    persist_settings = true;
                }
                ZOOM_1_ID | ZOOM_2_ID | ZOOM_4_ID => {
                    settings.zoom = match id {
                        ZOOM_1_ID => 1,
                        ZOOM_2_ID => 2,
                        _ => 4,
                    };
                    recreate_window = true;
                    persist_settings = true;
                }
                C16_PALETTE_ID | C16_PALETTE_125_ID | C16_PALETTE_150_ID | LEGACY_PALETTE_ID => {
                    settings.palette = PALETTE_CHOICES
                        .iter()
                        .find(|(_, _, choice_id)| *choice_id == id)
                        .unwrap()
                        .0;
                    for (index, (choice, _, _)) in PALETTE_CHOICES.iter().enumerate() {
                        set_menu_checked(menus.palette_items[index], settings.palette == *choice);
                    }
                    persist_settings = true;
                }
                _ => {}
            }
        }
        let left_down = window.get_mouse_down(MouseButton::Left);
        if left_down && !left_was_down {
            if let Some(position) = window.get_mouse_pos(MouseMode::Discard) {
                match toolbar_action(position) {
                    Some(ToolbarAction::Fast) => toggle_fast_mode = true,
                    Some(ToolbarAction::Sound) => toggle_sound = true,
                    Some(ToolbarAction::Colors) => open_colors = true,
                    None if settings.show_status => {
                        let (_, video_height) = settings.border.dimensions();
                        if realtime_text_hit(
                            position,
                            TOOLBAR_HEIGHT + video_height * settings.zoom as usize,
                            mips,
                            realtime,
                        ) {
                            toggle_fast_mode = true;
                        }
                    }
                    None => {}
                }
            }
        }
        left_was_down = left_down;
        if toggle_sound {
            if settings.sound_enabled {
                machine
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .set_audio_sample_rate(0);
                *audio.lock().unwrap_or_else(|e| e.into_inner()) = None;
                settings.sound_enabled = false;
                persist_settings = true;
            } else {
                match AudioOutput::open() {
                    Ok(output) => {
                        machine
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .set_audio_sample_rate(output.sample_rate());
                        *audio.lock().unwrap_or_else(|e| e.into_inner()) = Some(output);
                        settings.sound_enabled = true;
                        persist_settings = true;
                    }
                    Err(error) => eprintln!("sound unavailable: {error}"),
                }
            }
            set_menu_checked(menus.sound_item, settings.sound_enabled);
        }
        if open_colors && colors_dialog.is_none() {
            colors_dialog = Some(ColorsDialog::new(&window)?);
        } else if open_colors {
            #[cfg(target_os = "macos")]
            if let Some(dialog) = colors_dialog.as_ref() {
                macos_menu::focus_window(dialog.window.get_window_handle());
            }
        }
        let mut close_colors = false;
        if let Some(dialog) = colors_dialog.as_mut() {
            match dialog.update(&mut settings)? {
                None => close_colors = true,
                Some((changed, released)) => {
                    color_settings_dirty |= changed;
                    if released && color_settings_dirty {
                        persist_settings = true;
                        color_settings_dirty = false;
                    }
                }
            }
        }
        if close_colors {
            colors_dialog = None;
            if color_settings_dirty {
                persist_settings = true;
                color_settings_dirty = false;
            }
        }
        if toggle_fast_mode {
            settings.fast_mode = !settings.fast_mode;
            input.fast_mode.store(settings.fast_mode, Ordering::Relaxed);
            set_menu_checked(menus.fast_mode_item, settings.fast_mode);
            persist_settings = true;
            let machine = machine.lock().unwrap_or_else(|e| e.into_inner());
            meter_ticks = machine.ted.ticks;
            meter_instructions = machine.instructions_executed;
            meter_time = Instant::now();
        }
        if persist_settings {
            save_settings(&config_path, settings);
        }
        if recreate_window {
            let position = window.get_position();
            // NSApp's menu bar outlives a minifb window, so detach these
            // menus explicitly before replacing the window.
            menus.remove(&mut window);
            drop(window);
            let (new_width, new_height) = frame_dimensions(settings);
            rgb = vec![0u32; new_width * new_height];
            (window, menus) = create_window(settings)?;
            window.set_position(position.0, position.1);
            #[cfg(target_os = "macos")]
            {
                activate_after_first_event = true;
            }
        }
        if window.is_key_pressed(Key::F12, minifb::KeyRepeat::No) {
            let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
            input
                .paste
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear(&mut machine);
            machine.reset();
            if let Some(output) = audio.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                output.clear();
            }
            meter_ticks = machine.ted.ticks;
            meter_instructions = machine.instructions_executed;
            meter_time = Instant::now();
        }
        let run_stop_pressed = window.is_key_pressed(Key::Pause, minifb::KeyRepeat::No)
            || window.is_key_pressed(Key::F15, minifb::KeyRepeat::No);
        if run_stop_pressed {
            // A short host key event must survive until the ROM next scans the
            // keyboard; macOS may deliver press and release in one UI update.
            input.run_stop_frames.store(3, Ordering::Relaxed);
            let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
            input
                .paste
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear(&mut machine);
        }
        let command_down = if cfg!(target_os = "macos") {
            window.is_key_down(Key::LeftSuper) || window.is_key_down(Key::RightSuper)
        } else {
            window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl)
        };
        let shift_down = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        let commodore_down = window.is_key_down(Key::LeftAlt) || window.is_key_down(Key::RightAlt);
        let charset_combo_down = shift_down && commodore_down;
        if charset_combo_down && !charset_combo_was_down {
            // Keep queued clipboard keys from delaying the physical shortcut.
            let mut machine = machine.lock().unwrap_or_else(|e| e.into_inner());
            input
                .paste
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear(&mut machine);
        }
        charset_combo_was_down = charset_combo_down;
        *input.held_keys.lock().unwrap_or_else(|e| e.into_inner()) = window.get_keys();
        input.host_shift.store(shift_down, Ordering::Relaxed);
        input.run_stop_held.store(
            window.is_key_down(Key::Pause) || window.is_key_down(Key::F15),
            Ordering::Relaxed,
        );
        let mut machine_input = machine.lock().unwrap_or_else(|e| e.into_inner());
        for key in window.get_keys_pressed(minifb::KeyRepeat::No) {
            // The host clipboard shortcut belongs to Edit, not the C16 V key.
            if key == Key::V && command_down {
                continue;
            }
            if let Some(index) = matrix_key(key) {
                machine_input.set_key(index, true);
            }
        }
        for key in window.get_keys_released() {
            if let Some(index) = matrix_key(key) {
                machine_input.set_key(index, false);
            }
        }
        // Deliver RUN/STOP immediately, even when clipboard text is
        // still waiting in the keyboard queue. AppKit can call
        // Break F15 on external keyboards; other backends call it Pause.
        // The worker applies RUN/STOP and pasted keys at each emulated frame
        // boundary, so these inputs continue while AppKit tracks a menu.
        machine_input.set_key(61, commodore_down);
        drop(machine_input);
        let now = Instant::now();
        let elapsed = now.duration_since(meter_time).as_secs_f64();
        let (instructions_executed, ted_ticks) = {
            let machine = machine.lock().unwrap_or_else(|e| e.into_inner());
            (machine.instructions_executed, machine.ted.ticks)
        };
        if elapsed >= 0.5 {
            let instructions = instructions_executed - meter_instructions;
            let ticks = ted_ticks - meter_ticks;
            mips = instructions as f64 / elapsed / 1_000_000.0;
            realtime = ticks as f64 / PAL_TED_TICKS_PER_SECOND as f64 / elapsed;
            meter_time = now;
            meter_instructions = instructions_executed;
            meter_ticks = ted_ticks;
            if audio_debug {
                if let Some(output) = audio.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    let (queued, played_nonzero) = output.debug_counts();
                    eprintln!(
                        "audio: {queued} queued samples, {played_nonzero} nonzero samples played"
                    );
                }
            }
        }
        let (video_width, video_height) = settings.border.dimensions();
        let width = video_width * settings.zoom as usize;
        let video_height = video_height * settings.zoom as usize;
        let status_top = TOOLBAR_HEIGHT + video_height;
        let height = status_top + usize::from(settings.show_status) * STATUS_HEIGHT;
        debug_assert_eq!(rgb.len(), width * height);
        rgb.fill(0);
        draw_toolbar(&mut rgb, width, settings, colors_dialog.is_some());
        let pixels = machine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pixels_with_border(settings.border.pixels());
        draw_video(
            &mut rgb,
            width,
            &pixels,
            video_width,
            settings.zoom as usize,
            settings,
        );
        if settings.show_joystick_help {
            draw_joystick_help(&mut rgb, width, video_height);
        }
        if settings.show_status {
            draw_status(
                &mut rgb,
                width,
                status_top,
                mips,
                realtime,
                settings.sound_enabled,
            );
        }
        window
            .update_with_buffer(&rgb, width, height)
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        if activate_after_first_event {
            // Reassert visibility after the first event pump in case minifb's
            // deferred activation ran after we attached the menus.
            macos_menu::activate();
            activate_after_first_event = false;
        }
    }
    if color_settings_dirty {
        save_settings(&config_path, settings);
    }
    Ok(())
}

/// Print a clean diagnostic and exit nonzero on startup or emulation failure.
/// Arguments: none. Returns: nothing; exits nonzero on failure.
fn main() {
    if let Err(error) = run() {
        eprintln!("xc16emu: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Physical host keys always address the same C16 matrix positions.
    /// Arguments: none. Returns: nothing; assertions fail on key regressions.
    fn physical_keys_use_one_rom_agnostic_matrix() {
        assert_eq!(matrix_key(Key::Left), Some(48));
        assert_eq!(matrix_key(Key::Right), Some(51));
        assert_eq!(matrix_key(Key::Up), Some(43));
        assert_eq!(matrix_key(Key::Down), Some(40));
        assert_eq!(matrix_key(Key::Delete), Some(3));
        assert_eq!(matrix_key(Key::Backspace), Some(0));
        assert_eq!(matrix_key(Key::Key3), Some(8));
        assert_eq!(matrix_key(Key::LeftShift), Some(15));
        assert_eq!(matrix_key(Key::Pause), Some(63));
        assert_eq!(matrix_key(Key::F15), Some(63));
    }

    #[test]
    /// Shift with the physical 3 key reaches the ROM as C16 Shift+3.
    /// Arguments: none. Returns: nothing; assertions fail on matrix regressions.
    fn physical_shift_three_types_hash() {
        for roms in [
            concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms"),
        ] {
            let mut machine = Machine::from_rom_dir(roms).unwrap();
            machine.run_frames(100).unwrap();
            machine.set_key(matrix_key(Key::Key3).unwrap(), true);
            machine.set_key(matrix_key(Key::LeftShift).unwrap(), true);
            machine.run_frames(2).unwrap();
            machine.set_key(matrix_key(Key::Key3).unwrap(), false);
            machine.set_key(matrix_key(Key::LeftShift).unwrap(), false);
            machine.run_frames(2).unwrap();
            let screen = machine.capture_text_screen();
            assert!(screen.contains('#'), "{roms}: {screen}");
        }
    }

    #[test]
    /// A fast-mode key reaches BASIC once while its host key stays held.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn fast_mode_key_pulse_does_not_repeat() {
        for roms in [
            concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms"),
        ] {
            let mut machine = Machine::from_rom_dir(roms).unwrap();
            machine.run_frames(100).unwrap();
            machine.set_key(matrix_key(Key::A).unwrap(), true);
            machine.run_frames(1).unwrap();
            release_fast_mode_keys(&mut machine, [Key::A]);
            machine.run_frames(20).unwrap();
            let text = machine.capture_text_screen();
            assert!(
                text.lines().any(|line| line.trim() == "A"),
                "{roms}: {text}"
            );
        }
    }

    #[test]
    /// Verify emulation advances while the UI thread does no window work.
    /// Arguments: none. Returns: nothing; assertions fail if the worker stalls.
    fn emulator_worker_advances_without_ui_updates() {
        let machine = Arc::new(Mutex::new(
            Machine::from_rom_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms")).unwrap(),
        ));
        let audio = Arc::new(Mutex::new(None));
        let input = Arc::new(WorkerInput::default());
        let worker = EmulatorWorker::start(Arc::clone(&machine), audio, input, 1.0);
        std::thread::sleep(Duration::from_millis(75));
        let frames = machine.lock().unwrap().ted.frames;
        assert!(frames >= 2, "emulation stopped at {frames} frames");
        assert!(worker.error().is_none());
    }

    #[test]
    /// Verify Markdown BASIC lines are normalized and submitted to the ROM.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn paste_queue_types_basic_and_ignores_code_fences() {
        for roms in [
            concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms"),
        ] {
            let mut machine = Machine::from_rom_dir(roms).unwrap();
            machine.run_frames(100).unwrap();
            let mut paste = PasteQueue::default();
            paste
                .append("```basic\r\nPRINT 2+3*4 : REM test\r\n```\r\n")
                .unwrap();
            let frames = paste.pending.len() * 4;
            for _ in 0..frames {
                paste.step(&mut machine);
                machine.run_frames(1).unwrap();
            }
            let text = machine.capture_text_screen();
            assert!(text.contains(" 14 "), "{roms}: {text}");
            assert!(!text.contains("?SYNTAX ERROR"), "{roms}: {text}");
        }
    }

    #[test]
    /// Check settings serialization and invalid-input fallback.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn desktop_settings_round_trip_and_reject_invalid_values() {
        let settings = DesktopSettings {
            zoom: 4,
            border: BorderSize::Compact,
            palette: Palette::Legacy,
            brightness: 35,
            contrast: 125,
            show_status: false,
            show_joystick_help: true,
            fast_mode: true,
            sound_enabled: false,
        };
        assert_eq!(DesktopSettings::parse_toml(&settings.to_toml()), settings);
        let invalid = DesktopSettings::parse_toml(
            "zoom = 3\nborder = \"unknown\"\nstatus_bar = maybe\nfast_mode = false\n",
        );
        assert_eq!(invalid.zoom, 2);
        assert_eq!(invalid.border, BorderSize::Big);
        assert_eq!(invalid.palette, Palette::C16Colors);
        assert_eq!(invalid.brightness, 0);
        assert_eq!(invalid.contrast, 100);
        assert!(invalid.show_status);
        assert!(!invalid.show_joystick_help);
        assert!(!invalid.fast_mode);
        assert!(invalid.sound_enabled);
        assert!(DesktopSettings::parse_toml("free_run = true").fast_mode);
        for palette in [Palette::C16Colors125, Palette::C16Colors150] {
            let parsed = DesktopSettings::parse_toml(&format!("palette = \"{}\"", palette.name()));
            assert_eq!(parsed.palette, palette);
        }
        let invalid_color = DesktopSettings::parse_toml("brightness = 150\ncontrast = 20\n");
        assert_eq!((invalid_color.brightness, invalid_color.contrast), (0, 100));
    }

    #[test]
    /// Verify color controls brighten highlights and leave defaults unchanged.
    /// Arguments: none. Returns: nothing; assertions fail on color regressions.
    fn toolbar_and_color_adjustments() {
        assert_eq!(toolbar_action((20.0, 10.0)), Some(ToolbarAction::Fast));
        assert_eq!(toolbar_action((110.0, 10.0)), Some(ToolbarAction::Sound));
        assert_eq!(toolbar_action((200.0, 10.0)), Some(ToolbarAction::Colors));
        assert_eq!(toolbar_action((20.0, TOOLBAR_HEIGHT as f32)), None);
        assert_eq!(slider_value(0.0, -100, 100), -100);
        assert_eq!(slider_value(0.5, -100, 100), 0);
        assert_eq!(slider_value(1.0, 50, 200), 200);
        assert_eq!(adjust_rgb(0x7dcc96, 0, 100), 0x7dcc96);
        assert_eq!(adjust_rgb(0xe1e1e1, 100, 100), 0xffffff);
        assert_eq!(adjust_rgb(0x000000, 100, 100), 0x000000);
        assert_eq!(adjust_rgb(0x406080, 0, 200), 0x004080);
        let mut settings = DesktopSettings::default();
        let mut toolbar = vec![0u32; 336 * TOOLBAR_HEIGHT];
        draw_toolbar(&mut toolbar, 336, settings, false);
        assert_eq!(toolbar[0], 0x1b2028);
        let mut dialog_frame = vec![0; COLOR_DIALOG_WIDTH * COLOR_DIALOG_HEIGHT];
        let address = dialog_frame.as_ptr();
        render_colors_dialog(settings, &mut dialog_frame);
        assert_eq!(
            dialog_frame[153 * COLOR_DIALOG_WIDTH + 20],
            settings.palette.rgb(1)
        );
        assert_eq!(
            dialog_frame[153 * COLOR_DIALOG_WIDTH + 320],
            settings.palette.rgb(0)
        );
        let original_swatch = dialog_frame[153 * COLOR_DIALOG_WIDTH + 20];
        settings.brightness = 100;
        render_colors_dialog(settings, &mut dialog_frame);
        assert_eq!(dialog_frame.as_ptr(), address);
        assert_ne!(dialog_frame[153 * COLOR_DIALOG_WIDTH + 20], original_swatch);
        for label in [
            "FAST",
            "SOUND",
            "COLORS",
            "BRIGHTNESS",
            "CONTRAST",
            "PALETTE PREVIEW",
            "CHANGES APPLY LIVE",
        ] {
            assert!(label
                .chars()
                .filter(|c| *c != ' ')
                .all(|c| status_glyph(c) != [0; 7]));
        }
    }

    #[test]
    /// Check zoom scales video but keeps status at native resolution.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn video_zoom_leaves_status_font_and_outline_at_one_pixel() {
        let zoom = 4;
        let width = 320 * zoom;
        let video_height = 200 * zoom;
        let status_top = TOOLBAR_HEIGHT + video_height;
        let mut buffer = vec![0; width * (status_top + STATUS_HEIGHT)];
        let pixels = vec![1; 320 * 200];
        draw_video(
            &mut buffer,
            width,
            &pixels,
            320,
            zoom,
            DesktopSettings::default(),
        );
        draw_status(&mut buffer, width, status_top, 1.007, 1.0, true);
        assert_eq!(buffer[(status_top - 1) * width], Machine::rgb(1));
        assert!(buffer[status_top * width..(status_top + 1) * width]
            .iter()
            .all(|&pixel| pixel == 0x555555));
        assert_eq!(buffer[(status_top + 1) * width + 1], 0);
        assert_eq!(buffer[(status_top + 2) * width + 4], 0);
        assert_eq!(buffer[(status_top + 2) * width + 10], 0x888888);
        assert_eq!(buffer[(status_top + 9) * width + 10], 0);
        assert_eq!(buffer[(status_top + 10) * width + 10], 0x555555);
        assert_eq!(buffer[(status_top + 2) * width + width - 20 + 2], 0xffffff);
        let mut muted = vec![0; width * (status_top + STATUS_HEIGHT)];
        draw_status(&mut muted, width, status_top, 1.007, 1.0, false);
        assert_eq!(muted[(status_top + 2) * width + width - 20 + 2], 0x888888);
    }

    #[test]
    /// Check the REALTIME click target follows its displayed position and excludes video.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn realtime_click_target_matches_status_text() {
        let video_height = 200 * 4;
        let range = realtime_text_range(1.007, 1.0);
        assert!(realtime_text_hit(
            (range.start as f32, (video_height + 2) as f32),
            video_height,
            1.007,
            1.0,
        ));
        assert!(realtime_text_hit(
            ((range.end - 1) as f32, (video_height + 8) as f32),
            video_height,
            1.007,
            1.0,
        ));
        assert!(!realtime_text_hit(
            (range.start as f32, (video_height - 1) as f32),
            video_height,
            1.007,
            1.0,
        ));
        assert!(!realtime_text_hit(
            (range.end as f32, (video_height + 2) as f32),
            video_height,
            1.007,
            1.0,
        ));
    }

    #[test]
    /// Check help overlay fits compact video without touching status.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn joystick_help_fits_compact_video_and_leaves_status_clear() {
        let width = 336;
        let video_height = 216;
        let mut buffer = vec![0x123456; width * (TOOLBAR_HEIGHT + video_height + STATUS_HEIGHT)];
        draw_joystick_help(&mut buffer, width, video_height);
        assert!(buffer[..width * (TOOLBAR_HEIGHT + video_height)].contains(&0xffffff));
        assert!(buffer[..width * (TOOLBAR_HEIGHT + video_height)].contains(&0x7fc5ff));
        assert!(buffer[width * (TOOLBAR_HEIGHT + video_height)..]
            .iter()
            .all(|&pixel| pixel == 0x123456));
        for label in [
            "JOYSTICK KEYS",
            "PORT 1",
            "UP 5   DOWN R",
            "LEFT D RIGHT 6",
            "FIRE T",
            "PORT 2",
            "UP 3   DOWN W",
            "LEFT A RIGHT 4",
            "FIRE SHIFT",
            "USE MENU TO CLOSE",
        ] {
            assert!(label
                .chars()
                .filter(|c| *c != ' ')
                .all(|c| status_glyph(c) != [0; 7]));
        }
    }
}
