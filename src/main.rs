// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Desktop window and headless capture commands for the Rust emulator.

mod audio_output;

use audio_output::AudioOutput;
use minifb::{Key, MenuItemHandle, MouseButton, MouseMode, Scale, Window, WindowOptions};
#[cfg(not(target_os = "macos"))]
use minifb::{Menu, MenuHandle};
use std::{
    env, fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use xc16emu::{Machine, PAL_TED_TICKS_PER_SECOND};

/// One-pixel outline, one-pixel inset, seven-pixel font, and bottom inset.
const STATUS_HEIGHT: usize = 11;
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
            "zoom = {}\nborder = \"{}\"\nstatus_bar = {}\njoystick_help = {}\nfast_mode = {}\nsound = {}\n",
            self.zoom, border, self.show_status, self.show_joystick_help, self.fast_mode, self.sound_enabled
        )
    }
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
    top_items: [*mut std::ffi::c_void; 3],
    #[cfg(not(target_os = "macos"))]
    handles: [MenuHandle; 3],
    fast_mode_item: MenuItemHandle,
    sound_item: MenuItemHandle,
    joystick_help_item: MenuItemHandle,
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
/// Arguments: settings controls zoom, border, and status height. Returns: window and menu handles or an error.
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
    // The emulator's own PAL pacing controls the default rate. In Fast Mode,
    // minifb must also avoid its independent update-loop rate limit.
    window.set_target_fps(0);
    let menus = build_menus(&mut window, settings)?;
    Ok((window, menus))
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
    } = settings;
    let machine = macos_menu::new("Machine")?;
    macos_menu::item(machine, "Reset", RESET_ID)?;
    macos_menu::separator(machine);
    let fast_mode_item = macos_menu::item(machine, "Fast Mode", FAST_MODE_ID)?;
    set_menu_checked(fast_mode_item, fast_mode);
    macos_menu::separator(machine);
    let sound_item = macos_menu::item(machine, "Sound", SOUND_ID)?;
    set_menu_checked(sound_item, sound_enabled);
    let machine_top = macos_menu::attach(machine)?;

    let view = macos_menu::new("View")?;
    let status_item = macos_menu::item(view, "Show Status Bar", STATUS_ID)?;
    set_menu_checked(status_item, show_status);
    let joystick_help_item =
        macos_menu::item(view, "Show Joystick Keys (Break)", JOYSTICK_HELP_ID)?;
    set_menu_checked(joystick_help_item, show_joystick_help);
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
        fast_mode_item,
        sound_item,
        joystick_help_item,
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
    } = settings;
    let mut machine_menu = Menu::new("Machine").map_err(|e| e.to_string())?;
    machine_menu.add_item("Reset", RESET_ID).build();
    machine_menu.add_separator();
    let fast_mode_item = machine_menu.add_item("Fast Mode", FAST_MODE_ID).build();
    set_menu_checked(fast_mode_item, fast_mode);
    machine_menu.add_separator();
    let sound_item = machine_menu.add_item("Sound", SOUND_ID).build();
    set_menu_checked(sound_item, sound_enabled);
    let machine_handle = window.add_menu(&machine_menu);

    let mut view_menu = Menu::new("View").map_err(|e| e.to_string())?;
    let status_item = view_menu.add_item("Show Status Bar", STATUS_ID).build();
    set_menu_checked(status_item, show_status);
    let joystick_help_item = view_menu
        .add_item("Show Joystick Keys (Break)", JOYSTICK_HELP_ID)
        .build();
    set_menu_checked(joystick_help_item, show_joystick_help);
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
        fast_mode_item,
        sound_item,
        joystick_help_item,
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
        #[cfg(debug_assertions)]
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

    /// Catch menu setup regressions during desktop development. System Window
    /// commands may grow later, but must never land in Machine or View.
    /// Arguments: machine, view, and window are native menu pointers. Returns: nothing; debug assertions may fail.
    pub fn debug_check_layout(machine: Object, view: Object, window: Object) {
        #[cfg(debug_assertions)]
        unsafe {
            let count = sel_registerName(c"numberOfItems".as_ptr());
            debug_assert_eq!(message_count(machine, count), 5);
            debug_assert_eq!(message_count(view, count), 10);
            debug_assert_eq!(
                message_0(application(), sel_registerName(c"windowsMenu".as_ptr())),
                window
            );
        }
        #[cfg(not(debug_assertions))]
        let _ = (machine, view, window);
    }

    /// Remove the three native top-level menu items and clear AppKit's Window menu.
    /// Arguments: top_items contains Machine, View, and Window item pointers. Returns: nothing.
    pub fn remove(top_items: [Object; 3]) {
        set_window_menu(std::ptr::null_mut());
        unsafe {
            for item in top_items.into_iter().rev() {
                message_1(main_menu(), sel_registerName(c"removeItem:".as_ptr()), item);
            }
        }
    }
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
    let y = (video_height - PANEL_HEIGHT) / 2;
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
        (173, "BREAK TO CLOSE", 0x9ba9b5),
    ] {
        draw_help_text(buffer, width, text_x, y + offset, label, color);
    }
}

/// Scale only the C16 picture; status pixels are composed afterward at 1×.
/// Arguments: buffer is the output; width is row stride; pixels are TED colors; video_width and zoom set layout. Returns: nothing.
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
                println!("Usage: xc16emu [--roms DIR] [--drive DIR] [--dry-run] [--ram 16|32|64] [--prg FILE | --run-prg FILE] [--preload-prg FILE | --run-preload-prg FILE] [--go HEX] [--frozen-image FILE] [--type 'BASIC line'] [--headless --frames N] [--text] [--graphics] [--zoom 1|2|4] [--speed FACTOR] [--small-border]\n\n--prg loads after BASIC startup; --preload-prg loads before it. The --run variants start execution at the PRG's load address after startup. --go overrides the execution address (for example, --go 4000). Desktop settings are saved in ~/.xc16emu.toml. The Machine menu has Reset and Fast Mode. Click REALTIME in the status bar to toggle Fast Mode. The View menu controls the status bar, border size, zoom, and joystick key help. Escape closes the window; Break/Pause (F15 on macOS) toggles joystick help; F12 resets the machine.");
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
    for line in typed {
        machine.type_text(&line)?;
        machine.type_text("\n")?;
    }
    if let Some((path, run_loaded_prg)) = prg {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        let address = machine.load_prg(&data).map_err(str::to_string)?;
        if run_loaded_prg {
            run_address = Some(address);
        }
    }
    if let Some(address) = go_address.or(run_address) {
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
    let mut audio = if settings.sound_enabled {
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
    let (mut window, mut menus) = create_window(settings)?;
    if !config_path.exists() {
        save_settings(&config_path, settings);
    }
    let mut next_frame = Instant::now();
    let mut meter_time = next_frame;
    let mut meter_instructions = machine.instructions_executed;
    let mut meter_ticks = machine.ted.ticks;
    let mut paced_ticks = machine.ted.ticks;
    let mut mips = 0.0;
    let mut realtime = 0.0;
    let mut left_was_down = false;
    let audio_debug = env::var_os("XC16EMU_AUDIO_DEBUG").is_some();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // A setting that changes the video crop or status area changes the
        // fixed-size minifb buffer, so it requires a new window.
        let mut recreate_window = false;
        let mut persist_settings = false;
        let mut toggle_fast_mode = false;
        if let Some(id) = window.is_menu_pressed() {
            match id {
                RESET_ID => {
                    machine.reset();
                    if let Some(audio) = &audio {
                        audio.clear();
                    }
                    meter_ticks = machine.ted.ticks;
                    paced_ticks = machine.ted.ticks;
                    meter_instructions = machine.instructions_executed;
                    meter_time = Instant::now();
                    next_frame = meter_time;
                }
                FAST_MODE_ID => toggle_fast_mode = true,
                SOUND_ID => {
                    if settings.sound_enabled {
                        machine.set_audio_sample_rate(0);
                        audio = None;
                        settings.sound_enabled = false;
                        persist_settings = true;
                    } else {
                        match AudioOutput::open() {
                            Ok(output) => {
                                machine.set_audio_sample_rate(output.sample_rate());
                                audio = Some(output);
                                settings.sound_enabled = true;
                                persist_settings = true;
                            }
                            Err(error) => eprintln!("sound unavailable: {error}"),
                        }
                    }
                    set_menu_checked(menus.sound_item, settings.sound_enabled);
                }
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
        let left_down = window.get_mouse_down(MouseButton::Left);
        if left_down && !left_was_down && settings.show_status {
            let (_, video_height) = settings.border.dimensions();
            if window
                .get_mouse_pos(MouseMode::Discard)
                .is_some_and(|position| {
                    realtime_text_hit(
                        position,
                        video_height * settings.zoom as usize,
                        mips,
                        realtime,
                    )
                })
            {
                toggle_fast_mode = true;
            }
        }
        left_was_down = left_down;
        if toggle_fast_mode {
            settings.fast_mode = !settings.fast_mode;
            set_menu_checked(menus.fast_mode_item, settings.fast_mode);
            persist_settings = true;
            next_frame = Instant::now();
            meter_ticks = machine.ted.ticks;
            paced_ticks = machine.ted.ticks;
            meter_instructions = machine.instructions_executed;
            meter_time = next_frame;
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
            meter_ticks = machine.ted.ticks;
            paced_ticks = machine.ted.ticks;
            meter_instructions = machine.instructions_executed;
            meter_time = Instant::now();
            next_frame = meter_time;
        }
        // macOS reports the Break key on many external keyboards as F15;
        // other minifb backends report Pause directly.
        if window.is_key_pressed(Key::Pause, minifb::KeyRepeat::No)
            || window.is_key_pressed(Key::F15, minifb::KeyRepeat::No)
        {
            settings.show_joystick_help = !settings.show_joystick_help;
            set_menu_checked(menus.joystick_help_item, settings.show_joystick_help);
            save_settings(&config_path, settings);
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
            let ticks = machine.ted.ticks - meter_ticks;
            mips = instructions as f64 / elapsed / 1_000_000.0;
            realtime = ticks as f64 / PAL_TED_TICKS_PER_SECOND as f64 / elapsed;
            meter_time = now;
            meter_instructions = machine.instructions_executed;
            meter_ticks = machine.ted.ticks;
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
        if settings.show_joystick_help {
            draw_joystick_help(&mut rgb, width, video_height);
        }
        if settings.show_status {
            draw_status(
                &mut rgb,
                width,
                video_height,
                mips,
                realtime,
                settings.sound_enabled,
            );
        }
        window
            .update_with_buffer(&rgb, width, height)
            .map_err(|e| e.to_string())?;
        if settings.fast_mode {
            next_frame = Instant::now();
            paced_ticks = machine.ted.ticks;
        } else {
            let ticks = machine.ted.ticks - paced_ticks;
            paced_ticks = machine.ted.ticks;
            next_frame +=
                Duration::from_secs_f64(ticks as f64 / PAL_TED_TICKS_PER_SECOND as f64 / speed);
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
    /// Check settings serialization and invalid-input fallback.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn desktop_settings_round_trip_and_reject_invalid_values() {
        let settings = DesktopSettings {
            zoom: 4,
            border: BorderSize::Compact,
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
        assert!(invalid.show_status);
        assert!(!invalid.show_joystick_help);
        assert!(!invalid.fast_mode);
        assert!(invalid.sound_enabled);
        assert!(DesktopSettings::parse_toml("free_run = true").fast_mode);
    }

    #[test]
    /// Check zoom scales video but keeps status at native resolution.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn video_zoom_leaves_status_font_and_outline_at_one_pixel() {
        let zoom = 4;
        let width = 320 * zoom;
        let video_height = 200 * zoom;
        let mut buffer = vec![0; width * (video_height + STATUS_HEIGHT)];
        let pixels = vec![1; 320 * 200];
        draw_video(&mut buffer, width, &pixels, 320, zoom);
        draw_status(&mut buffer, width, video_height, 1.007, 1.0, true);
        assert_eq!(buffer[(video_height - 1) * width], Machine::rgb(1));
        assert!(buffer[video_height * width..(video_height + 1) * width]
            .iter()
            .all(|&pixel| pixel == 0x555555));
        assert_eq!(buffer[(video_height + 1) * width + 1], 0);
        assert_eq!(buffer[(video_height + 2) * width + 4], 0);
        assert_eq!(buffer[(video_height + 2) * width + 10], 0x888888);
        assert_eq!(buffer[(video_height + 9) * width + 10], 0);
        assert_eq!(buffer[(video_height + 10) * width + 10], 0x555555);
        assert_eq!(
            buffer[(video_height + 2) * width + width - 20 + 2],
            0xffffff
        );
        let mut muted = vec![0; width * (video_height + STATUS_HEIGHT)];
        draw_status(&mut muted, width, video_height, 1.007, 1.0, false);
        assert_eq!(muted[(video_height + 2) * width + width - 20 + 2], 0x888888);
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
        let mut buffer = vec![0x123456; width * (video_height + STATUS_HEIGHT)];
        draw_joystick_help(&mut buffer, width, video_height);
        assert!(buffer[..width * video_height].contains(&0xffffff));
        assert!(buffer[..width * video_height].contains(&0x7fc5ff));
        assert!(buffer[width * video_height..]
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
            "BREAK TO CLOSE",
        ] {
            assert!(label
                .chars()
                .filter(|c| *c != ' ')
                .all(|c| status_glyph(c) != [0; 7]));
        }
    }
}
