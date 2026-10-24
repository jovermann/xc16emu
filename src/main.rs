// Copyright (C) 2026 Johannes Overmann and contributors.
// SPDX-License-Identifier: GPL-2.0-or-later
//! Desktop window and headless capture commands for the Rust emulator.

mod audio_output;

use audio_output::AudioOutput;
use minifb::{Key, MenuItemHandle, Scale, Window, WindowOptions};
#[cfg(not(target_os = "macos"))]
use minifb::{Menu, MenuHandle};
use std::{
    env, fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use xc16emu::Machine;

/// One-pixel outline, one-pixel inset, seven-pixel font, and bottom inset.
const STATUS_HEIGHT: usize = 11;
const RESET_ID: usize = 1;
const FREE_RUN_ID: usize = 2;
const STATUS_ID: usize = 3;
const BIG_BORDER_ID: usize = 4;
const MEDIUM_BORDER_ID: usize = 5;
const COMPACT_BORDER_ID: usize = 6;
const ZOOM_1_ID: usize = 7;
const ZOOM_2_ID: usize = 8;
const ZOOM_4_ID: usize = 9;

/// The border widths are measured in emulator pixels on every side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorderSize {
    Big,
    Medium,
    Compact,
}

impl BorderSize {
    fn pixels(self) -> usize {
        match self {
            Self::Big => 32,
            Self::Medium => 16,
            Self::Compact => 8,
        }
    }

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
    show_status: bool,
    free_run: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            zoom: 2,
            border: BorderSize::Big,
            show_status: true,
            free_run: false,
        }
    }
}

impl DesktopSettings {
    /// Read the small subset of TOML used by this file. Invalid or unknown
    /// values leave their defaults in place, so hand edits cannot break boot.
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
                "status_bar" => match value {
                    "true" => settings.show_status = true,
                    "false" => settings.show_status = false,
                    _ => {}
                },
                "free_run" => match value {
                    "true" => settings.free_run = true,
                    "false" => settings.free_run = false,
                    _ => {}
                },
                _ => {}
            }
        }
        settings
    }

    fn to_toml(self) -> String {
        let border = match self.border {
            BorderSize::Big => "big",
            BorderSize::Medium => "medium",
            BorderSize::Compact => "compact",
        };
        format!(
            "zoom = {}\nborder = \"{}\"\nstatus_bar = {}\nfree_run = {}\n",
            self.zoom, border, self.show_status, self.free_run
        )
    }
}

fn settings_path() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".xc16emu.toml"))
        .ok_or("HOME is not set; cannot locate ~/.xc16emu.toml".to_string())
}

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

fn save_settings(path: &std::path::Path, settings: DesktopSettings) {
    if let Err(error) = fs::write(path, settings.to_toml()) {
        eprintln!("xc16emu: could not save {}: {error}", path.display());
    }
}

/// Give a native macOS menu item a real checkmark. minifb exposes its Cocoa
/// menu-item pointer but does not provide an API for changing its state.
#[cfg(target_os = "macos")]
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
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_state(receiver: *mut c_void, selector: *mut c_void, state: isize);
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_object(
            receiver: *mut c_void,
            selector: *mut c_void,
            object: *mut c_void,
        ) -> *mut c_void;
        #[link_name = "objc_msgSend"]
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

/// Other window backends use a text checkmark instead.
#[cfg(not(target_os = "macos"))]
fn set_menu_checked(_item: MenuItemHandle, _checked: bool) {}

/// The menu handles permit clean replacement when the display size changes.
struct WindowMenus {
    #[cfg(target_os = "macos")]
    top_items: [*mut std::ffi::c_void; 3],
    #[cfg(not(target_os = "macos"))]
    handles: [MenuHandle; 3],
    free_run_item: MenuItemHandle,
}

impl WindowMenus {
    /// Remove this window's top-level menus before changing the display size.
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
fn create_window(settings: DesktopSettings) -> Result<(Window, WindowMenus), String> {
    let DesktopSettings {
        zoom,
        border,
        show_status,
        ..
    } = settings;
    let (video_width, video_height) = border.dimensions();
    let width = video_width * zoom as usize;
    let height = video_height * zoom as usize + usize::from(show_status) * STATUS_HEIGHT;
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
    // The emulator's own PAL pacing controls the default rate. In Free Run,
    // minifb must also avoid its independent update-loop rate limit.
    window.set_target_fps(0);
    let menus = build_menus(&mut window, settings)?;
    Ok((window, menus))
}

/// On macOS, attach Machine and View without minifb's `add_menu`, which
/// incorrectly designates each one as AppKit's Window menu.
#[cfg(target_os = "macos")]
fn build_menus(_window: &mut Window, settings: DesktopSettings) -> Result<WindowMenus, String> {
    let DesktopSettings {
        zoom,
        border,
        show_status,
        free_run,
    } = settings;
    let machine = macos_menu::new("Machine")?;
    macos_menu::item(machine, "Reset", RESET_ID)?;
    macos_menu::separator(machine);
    let free_run_item = macos_menu::item(machine, "Free Run", FREE_RUN_ID)?;
    set_menu_checked(free_run_item, free_run);
    let machine_top = macos_menu::attach(machine)?;

    let view = macos_menu::new("View")?;
    let status_item = macos_menu::item(view, "Show Status Bar", STATUS_ID)?;
    set_menu_checked(status_item, show_status);
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
    let view_top = macos_menu::attach(view)?;

    let window_menu = macos_menu::new("Window")?;
    let window_top = macos_menu::attach(window_menu)?;
    macos_menu::set_window_menu(window_menu);
    macos_menu::debug_check_layout(machine, view, window_menu);
    Ok(WindowMenus {
        top_items: [machine_top, view_top, window_top],
        free_run_item,
    })
}

/// Other minifb backends can use its ordinary menu API.
#[cfg(not(target_os = "macos"))]
fn build_menus(window: &mut Window, settings: DesktopSettings) -> Result<WindowMenus, String> {
    let DesktopSettings {
        zoom,
        border,
        show_status,
        free_run,
    } = settings;
    let mut machine_menu = Menu::new("Machine").map_err(|e| e.to_string())?;
    machine_menu.add_item("Reset", RESET_ID).build();
    machine_menu.add_separator();
    let free_run_item = machine_menu.add_item("Free Run", FREE_RUN_ID).build();
    set_menu_checked(free_run_item, free_run);
    let machine_handle = window.add_menu(&machine_menu);

    let mut view_menu = Menu::new("View").map_err(|e| e.to_string())?;
    let status_item = view_menu.add_item("Show Status Bar", STATUS_ID).build();
    set_menu_checked(status_item, show_status);
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
    let view_handle = window.add_menu(&view_menu);

    let window_menu = Menu::new("Window").map_err(|e| e.to_string())?;
    let window_handle = window.add_menu(&window_menu);
    Ok(WindowMenus {
        handles: [machine_handle, view_handle, window_handle],
        free_run_item,
    })
}

/// Attach macOS menus without minifb's top-level attachment function. That
/// function calls `setWindowsMenu:` for every menu, which causes AppKit to
/// inject the system Window commands into Machine and View.
#[cfg(target_os = "macos")]
mod macos_menu {
    use super::MenuItemHandle;
    use std::ffi::{c_char, c_void, CString};

    type Object = *mut c_void;

    unsafe extern "C" {
        fn mfb_create_menu(name: *const c_char) -> Object;
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
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Object;
        #[link_name = "objc_msgSend"]
        fn message_0(receiver: Object, selector: Object) -> Object;
        #[link_name = "objc_msgSend"]
        #[cfg(debug_assertions)]
        fn message_count(receiver: Object, selector: Object) -> isize;
        #[link_name = "objc_msgSend"]
        fn message_cstr(receiver: Object, selector: Object, value: *const c_char) -> Object;
        #[link_name = "objc_msgSend"]
        fn message_1(receiver: Object, selector: Object, value: Object);
        #[link_name = "objc_msgSend"]
        fn message_bool(receiver: Object, selector: Object, value: isize);
        #[link_name = "objc_msgSend"]
        fn message_3(
            receiver: Object,
            selector: Object,
            first: Object,
            second: Object,
            third: Object,
        ) -> Object;
    }

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

    fn application() -> Object {
        unsafe {
            message_0(
                objc_getClass(c"NSApplication".as_ptr()),
                sel_registerName(c"sharedApplication".as_ptr()),
            )
        }
    }

    fn main_menu() -> Object {
        unsafe { message_0(application(), sel_registerName(c"mainMenu".as_ptr())) }
    }

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
    pub fn item(menu: Object, label: &str, id: usize) -> Result<MenuItemHandle, String> {
        let label = CString::new(label).map_err(|_| "menu item contains NUL".to_string())?;
        let handle = unsafe { mfb_add_menu_item(menu, id as i32, label.as_ptr(), true, 0x7f, 0) };
        if handle == 0 {
            Err("could not create macOS menu item".to_string())
        } else {
            Ok(MenuItemHandle(handle))
        }
    }

    pub fn separator(menu: Object) {
        unsafe {
            mfb_add_menu_item(menu, -1, c"".as_ptr(), true, 0x7f, 0);
        }
    }

    /// Add a top-level menu without changing `NSApp.windowsMenu`.
    pub fn attach(menu: Object) -> Result<Object, String> {
        let empty = string("")?;
        unsafe {
            let top = message_3(
                main_menu(),
                sel_registerName(c"addItemWithTitle:action:keyEquivalent:".as_ptr()),
                empty,
                std::ptr::null_mut(),
                empty,
            );
            if top.is_null() {
                return Err("could not attach macOS menu".to_string());
            }
            message_1(top, sel_registerName(c"setSubmenu:".as_ptr()), menu);
            message_bool(menu, sel_registerName(c"setAutoenablesItems:".as_ptr()), 0);
            Ok(top)
        }
    }

    pub fn set_window_menu(menu: Object) {
        unsafe {
            message_1(
                application(),
                sel_registerName(c"setWindowsMenu:".as_ptr()),
                menu,
            );
        }
    }

    /// Catch menu setup regressions during desktop development. System Window
    /// commands may grow later, but must never land in Machine or View.
    pub fn debug_check_layout(machine: Object, view: Object, window: Object) {
        #[cfg(debug_assertions)]
        unsafe {
            let count = sel_registerName(c"numberOfItems".as_ptr());
            debug_assert_eq!(message_count(machine, count), 3);
            debug_assert_eq!(message_count(view, count), 9);
            debug_assert_eq!(
                message_0(application(), sel_registerName(c"windowsMenu".as_ptr())),
                window
            );
        }
        #[cfg(not(debug_assertions))]
        let _ = (machine, view, window);
    }

    pub fn remove(top_items: [Object; 3]) {
        set_window_menu(std::ptr::null_mut());
        unsafe {
            for item in top_items.into_iter().rev() {
                message_1(main_menu(), sel_registerName(c"removeItem:".as_ptr()), item);
            }
        }
    }
}

/// Minimal five-by-seven glyphs for the status meter; no host font is needed.
fn status_glyph(character: char) -> [u8; 7] {
    match character {
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
        ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
        '.' => [0, 0, 0, 0, 0, 0b00100, 0b00100],
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

/// Scale only the C16 picture; status pixels are composed afterward at 1×.
fn draw_video(buffer: &mut [u32], width: usize, pixels: &[u8], video_width: usize, zoom: usize) {
    for (y, row) in pixels.chunks_exact(video_width).enumerate() {
        for (x, &color) in row.iter().enumerate() {
            let rgb = Machine::rgb(color);
            for dy in 0..zoom {
                let start = (y * zoom + dy) * width + x * zoom;
                buffer[start..start + zoom].fill(rgb);
            }
        }
    }
}

/// A one-pixel gray outline separates the black status strip from the video.
fn draw_status(buffer: &mut [u32], width: usize, video_height: usize, mips: f64, realtime: f64) {
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
    let realtime_x = mips_x + mips_value.len() * 6 + 18;
    draw_status_text(buffer, width, realtime_x, y, "REALTIME: ", 0x888888);
    draw_status_text(
        buffer,
        width,
        realtime_x + 10 * 6,
        y,
        &format!("{realtime:.2}X"),
        0xffffff,
    );
}

/// Parse a decimal frame count from a command-line option.
fn parse_frames(value: Option<String>) -> Result<u64, String> {
    value
        .ok_or("--frames needs a number".to_string())?
        .parse()
        .map_err(|_| "invalid frame count".to_string())
}

/// Monitor addresses are hexadecimal, with or without a $ or 0x prefix.
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
fn matrix_key(key: Key) -> Option<u8> {
    Some(match key {
        Key::Backspace | Key::Delete => 0,
        Key::Enter => 1,
        Key::F8 => 3,
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
        Key::Tab => 63,
        Key::LeftShift | Key::RightShift => 15,
        _ => return None,
    })
}

/// Parse options, boot the ROM, and run in capture or desktop mode.
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut roms = PathBuf::from("old/roms");
    let mut frames = 250u64;
    let mut headless = false;
    let mut text = false;
    let mut graphics = false;
    let mut prg = None;
    let mut module = None;
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
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--roms" => roms = PathBuf::from(args.next().ok_or("--roms needs a path")?),
            "--drive" => drive = PathBuf::from(args.next().ok_or("--drive needs a path")?),
            "--dry-run" => dry_run = true,
            "--verbose" | "-v" => verbose = true,
            "--small-border" => border_override = Some(BorderSize::Compact),
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
            "--prg" => prg = Some(PathBuf::from(args.next().ok_or("--prg needs a path")?)),
            "--module" => module = Some(PathBuf::from(args.next().ok_or("--module needs a path")?)),
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
                println!("Usage: xc16emu [--roms DIR] [--drive DIR] [--dry-run] [--ram 16|32|64] [--prg FILE] [--module FILE] [--go HEX] [--frozen-image FILE] [--type 'BASIC line'] [--headless --frames N] [--text] [--graphics] [--zoom 1|2|4] [--speed FACTOR] [--small-border]\n\n--module loads a two-byte-address image after ROM startup; --go starts execution at a hexadecimal address (for example, --go 4000). Desktop settings are saved in ~/.xc16emu.toml. The Machine menu has Reset and Free Run. The View menu controls the status bar, border size, and zoom. Escape closes the window; F12 resets the machine.");
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
    if let Some(path) = prg {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        machine.load_prg(&data).map_err(str::to_string)?;
    }
    machine.run_frames(100)?;
    for line in typed {
        machine.type_text(&line)?;
        machine.type_text("\n")?;
    }
    if let Some(path) = module {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        machine.load_prg(&data).map_err(str::to_string)?;
    }
    if let Some(address) = go_address {
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
    let (mut window, mut menus) = create_window(settings)?;
    let audio = match AudioOutput::open() {
        Ok(output) => {
            machine.set_audio_sample_rate(output.sample_rate());
            Some(output)
        }
        Err(error) => {
            eprintln!("sound unavailable: {error}");
            None
        }
    };
    if !config_path.exists() {
        save_settings(&config_path, settings);
    }
    let mut next_frame = Instant::now();
    let mut meter_time = next_frame;
    let mut meter_instructions = machine.instructions_executed;
    let mut meter_frames = machine.ted.frames;
    let mut mips = 0.0;
    let mut realtime = 0.0;
    let audio_debug = env::var_os("XC16EMU_AUDIO_DEBUG").is_some();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // A setting that changes the video crop or status area changes the
        // fixed-size minifb buffer, so it requires a new window.
        let mut recreate_window = false;
        let mut persist_settings = false;
        if let Some(id) = window.is_menu_pressed() {
            match id {
                RESET_ID => {
                    machine.reset();
                    if let Some(audio) = &audio {
                        audio.clear();
                    }
                    meter_frames = machine.ted.frames;
                    meter_instructions = machine.instructions_executed;
                    meter_time = Instant::now();
                }
                FREE_RUN_ID => {
                    settings.free_run = !settings.free_run;
                    set_menu_checked(menus.free_run_item, settings.free_run);
                    persist_settings = true;
                    next_frame = Instant::now();
                    meter_frames = machine.ted.frames;
                    meter_instructions = machine.instructions_executed;
                    meter_time = next_frame;
                }
                STATUS_ID => {
                    settings.show_status = !settings.show_status;
                    recreate_window = true;
                    persist_settings = true;
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
                _ => {}
            }
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
            (window, menus) = create_window(settings)?;
            window.set_position(position.0, position.1);
        }
        if window.is_key_pressed(Key::F12, minifb::KeyRepeat::No) {
            machine.reset();
            if let Some(audio) = &audio {
                audio.clear();
            }
            meter_frames = machine.ted.frames;
            meter_instructions = machine.instructions_executed;
            meter_time = Instant::now();
        }
        for key in window.get_keys_pressed(minifb::KeyRepeat::No) {
            if let Some(index) = matrix_key(key) {
                machine.set_key(index, true);
            }
        }
        for key in window.get_keys_released() {
            if let Some(index) = matrix_key(key) {
                machine.set_key(index, false);
            }
        }
        machine.run_frames(1)?;
        if let Some(audio) = &audio {
            audio.push(machine.drain_audio_samples());
        }
        let now = Instant::now();
        let elapsed = now.duration_since(meter_time).as_secs_f64();
        if elapsed >= 0.5 {
            let instructions = machine.instructions_executed - meter_instructions;
            let frames = machine.ted.frames - meter_frames;
            mips = instructions as f64 / elapsed / 1_000_000.0;
            // Each emulated PAL frame represents 20 ms of machine time.
            realtime = frames as f64 * 0.020 / elapsed;
            meter_time = now;
            meter_instructions = machine.instructions_executed;
            meter_frames = machine.ted.frames;
            if audio_debug {
                if let Some(audio) = &audio {
                    let (queued, played_nonzero) = audio.debug_counts();
                    eprintln!(
                        "audio: {queued} queued samples, {played_nonzero} nonzero samples played"
                    );
                }
            }
        }
        let (video_width, video_height) = settings.border.dimensions();
        let width = video_width * settings.zoom as usize;
        let video_height = video_height * settings.zoom as usize;
        let height = video_height + usize::from(settings.show_status) * STATUS_HEIGHT;
        let mut rgb = vec![0u32; width * height];
        let pixels = machine.pixels_with_border(settings.border.pixels());
        draw_video(
            &mut rgb,
            width,
            &pixels,
            video_width,
            settings.zoom as usize,
        );
        if settings.show_status {
            draw_status(&mut rgb, width, video_height, mips, realtime);
        }
        window
            .update_with_buffer(&rgb, width, height)
            .map_err(|e| e.to_string())?;
        if settings.free_run {
            next_frame = Instant::now();
        } else {
            next_frame += Duration::from_secs_f64(0.020 / speed);
            if let Some(wait) = next_frame.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            } else {
                next_frame = Instant::now();
            }
        }
    }
    Ok(())
}

/// Print a clean diagnostic and exit nonzero on startup or emulation failure.
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
    fn desktop_settings_round_trip_and_reject_invalid_values() {
        let settings = DesktopSettings {
            zoom: 4,
            border: BorderSize::Compact,
            show_status: false,
            free_run: true,
        };
        assert_eq!(DesktopSettings::parse_toml(&settings.to_toml()), settings);
        let invalid = DesktopSettings::parse_toml(
            "zoom = 3\nborder = \"unknown\"\nstatus_bar = maybe\nfree_run = false\n",
        );
        assert_eq!(invalid.zoom, 2);
        assert_eq!(invalid.border, BorderSize::Big);
        assert!(invalid.show_status);
        assert!(!invalid.free_run);
    }

    #[test]
    fn video_zoom_leaves_status_font_and_outline_at_one_pixel() {
        let zoom = 4;
        let width = 320 * zoom;
        let video_height = 200 * zoom;
        let mut buffer = vec![0; width * (video_height + STATUS_HEIGHT)];
        let pixels = vec![1; 320 * 200];
        draw_video(&mut buffer, width, &pixels, 320, zoom);
        draw_status(&mut buffer, width, video_height, 1.007, 1.0);
        assert_eq!(buffer[(video_height - 1) * width], Machine::rgb(1));
        assert!(buffer[video_height * width..(video_height + 1) * width]
            .iter()
            .all(|&pixel| pixel == 0x555555));
        assert_eq!(buffer[(video_height + 1) * width + 1], 0);
        assert_eq!(buffer[(video_height + 2) * width + 4], 0);
        assert_eq!(buffer[(video_height + 2) * width + 10], 0x888888);
        assert_eq!(buffer[(video_height + 9) * width + 10], 0);
        assert_eq!(buffer[(video_height + 10) * width + 10], 0x555555);
    }
}
