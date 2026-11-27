//! `gentle-screen` on macOS: windows from the window server
//! (`CGWindowListCopyWindowInfo`), pictures from `screencapture`, input as
//! Quartz events posted at the window's place after bringing its app
//! forward. Coordinates are points. Window titles and pictures need Screen
//! Recording permission, input needs Accessibility; both are granted to the
//! app the agent runs in (Osdy Code Desktop), which macOS asks about once.

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField,
    ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowLayer,
    kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowName,
    kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
};
use gentle_core::screen::{Button, Chord, Key, Window};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
}

/// On-screen app windows (layer 0), frontmost first, as the window server
/// orders them.
pub fn windows() -> Result<Vec<Window>, String> {
    let list = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
    )
    .ok_or("the window server gave no window list")?;
    // SAFETY: the window list holds dictionaries.
    let list: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { CFArray::wrap_under_get_rule(list.as_concrete_TypeRef()) };
    let mut windows = Vec::new();
    for info in list.iter() {
        // SAFETY: the keys are the window server's own constants.
        let key = |k| unsafe { CFString::wrap_under_get_rule(k) };
        let number = |k| {
            info.find(key(k))
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i64())
        };
        let text = |k| {
            info.find(key(k))
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string())
                .unwrap_or_default()
        };
        if number(unsafe { kCGWindowLayer }) != Some(0) {
            continue;
        }
        let (Some(id), Some(pid)) = (
            number(unsafe { kCGWindowNumber }),
            number(unsafe { kCGWindowOwnerPID }),
        ) else {
            continue;
        };
        let Some(bounds) = info
            .find(key(unsafe { kCGWindowBounds }))
            .and_then(|v| v.downcast::<CFDictionary>())
        else {
            continue;
        };
        // SAFETY: the bounds dictionary maps X, Y, Width, Height to numbers.
        let bounds: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_get_rule(bounds.as_concrete_TypeRef()) };
        let field = |name: &str| {
            bounds
                .find(CFString::new(name))
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_f64())
                .unwrap_or(0.)
        };
        let (width, height) = (field("Width"), field("Height"));
        // Menu-bar extras and invisible helpers come as tiny windows.
        if width < 40. || height < 40. {
            continue;
        }
        windows.push(Window {
            id: id as u64,
            pid: pid as u32,
            app: text(unsafe { kCGWindowOwnerName }),
            title: text(unsafe { kCGWindowName }),
            x: field("X"),
            y: field("Y"),
            width,
            height,
        });
    }
    Ok(windows)
}

pub fn capture_window(w: &Window) -> Result<Vec<u8>, String> {
    screencapture(&["-o", &format!("-l{}", w.id)])
}

pub fn capture_screen() -> Result<Vec<u8>, String> {
    screencapture(&[])
}

/// `screencapture -x <args> <file>`: its PNG. Without Screen Recording
/// permission macOS answers with the desktop only, or nothing.
fn screencapture(args: &[&str]) -> Result<Vec<u8>, String> {
    let path = std::env::temp_dir().join(format!("gentle-screen-raw-{}.png", std::process::id()));
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .args(args)
        .arg(&path)
        .status()
        .map_err(|e| e.to_string())?;
    let png = std::fs::read(&path).ok();
    let _ = std::fs::remove_file(&path);
    match png {
        Some(png) if status.success() => Ok(png),
        _ => Err("macOS gave no picture: allow Osdy Code Desktop in System Settings › Privacy & Security › Screen & System Audio Recording".into()),
    }
}

pub fn parent(pid: u32) -> Option<u32> {
    let out = std::process::Command::new("/bin/ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub fn alive(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Posting events needs Accessibility; asking also shows macOS's prompt.
pub fn input_ready() -> Result<(), String> {
    let options = CFDictionary::from_CFType_pairs(&[(
        CFString::new("AXTrustedCheckOptionPrompt"),
        CFBoolean::true_value(),
    )]);
    // SAFETY: a plain CF dictionary, as the call expects.
    if unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) } {
        Ok(())
    } else {
        Err("allow Osdy Code Desktop in System Settings › Privacy & Security › Accessibility, then try again".into())
    }
}

pub fn focus(w: &Window) -> Result<(), String> {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(w.pid as i32)
        .ok_or("the window's app is gone")?;
    // Already in front: activating again would make the next click only
    // bring its window forward in some apps.
    if app.isActive() {
        return Ok(());
    }
    #[allow(deprecated)]
    app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

fn source() -> Result<CGEventSource, String> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|()| "couldn't make an input event".to_string())
}

fn post(event: Result<CGEvent, ()>) -> Result<(), String> {
    event
        .map_err(|()| "couldn't make an input event".to_string())?
        .post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(12));
    Ok(())
}

pub fn click(w: &Window, x: f64, y: f64, button: Button, count: u8) -> Result<(), String> {
    let at = CGPoint::new(w.x + x, w.y + y);
    let (down, up, which) = match button {
        Button::Left => (
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGMouseButton::Left,
        ),
        Button::Right => (
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGMouseButton::Right,
        ),
        Button::Middle => (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGMouseButton::Center,
        ),
    };
    post(CGEvent::new_mouse_event(
        source()?,
        CGEventType::MouseMoved,
        at,
        which,
    ))?;
    // Apps miss a press that comes with the pointer's arrival, or a release
    // in the same instant as the press: a person takes tens of ms.
    std::thread::sleep(std::time::Duration::from_millis(60));
    for n in 1..=count.max(1) {
        for kind in [down, up] {
            let event = CGEvent::new_mouse_event(source()?, kind, at, which)
                .map_err(|()| "couldn't make an input event".to_string())?;
            // The click count makes the second of two a double click.
            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, i64::from(n));
            post(Ok(event))?;
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    }
    Ok(())
}

pub fn type_text(_w: &Window, text: &str) -> Result<(), String> {
    // Each character rides on a key event of its own: apps that read keys
    // (not text) take only the first character of a longer string.
    let mut buf = [0u16; 2];
    for c in text.chars() {
        let units = c.encode_utf16(&mut buf);
        for down in [true, false] {
            let event = CGEvent::new_keyboard_event(source()?, 0, down)
                .map_err(|()| "couldn't make an input event".to_string())?;
            event.set_string_from_utf16_unchecked(units);
            post(Ok(event))?;
        }
    }
    Ok(())
}

pub fn key(_w: &Window, chord: &Chord) -> Result<(), String> {
    let code = keycode(chord.key).ok_or_else(|| format!("no key for {:?} here", chord.key))?;
    let m = chord.modifiers;
    let mut flags = CGEventFlags::empty();
    let mut held: Vec<u16> = Vec::new();
    for (on, flag, key) in [
        (m.cmd, CGEventFlags::CGEventFlagCommand, 0x37),
        (m.ctrl, CGEventFlags::CGEventFlagControl, 0x3B),
        (m.alt, CGEventFlags::CGEventFlagAlternate, 0x3A),
        (m.shift, CGEventFlags::CGEventFlagShift, 0x38),
    ] {
        if on {
            flags |= flag;
            held.push(key);
            let event = CGEvent::new_keyboard_event(source()?, key, true)
                .map_err(|()| "couldn't make an input event".to_string())?;
            event.set_flags(flags);
            post(Ok(event))?;
        }
    }
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(source()?, code, down)
            .map_err(|()| "couldn't make an input event".to_string())?;
        event.set_flags(flags);
        post(Ok(event))?;
    }
    for key in held.into_iter().rev() {
        let event = CGEvent::new_keyboard_event(source()?, key, false)
            .map_err(|()| "couldn't make an input event".to_string())?;
        event.set_flags(CGEventFlags::empty());
        post(Ok(event))?;
    }
    Ok(())
}

pub fn scroll(w: &Window, x: f64, y: f64, dy: f64) -> Result<(), String> {
    post(CGEvent::new_mouse_event(
        source()?,
        CGEventType::MouseMoved,
        CGPoint::new(w.x + x, w.y + y),
        CGMouseButton::Left,
    ))?;
    // Positive wheel values scroll up on macOS.
    post(CGEvent::new_scroll_event(
        source()?,
        ScrollEventUnit::PIXEL,
        1,
        -(dy.round() as i32),
        0,
        0,
    ))
}

/// The virtual key code of `key` on an ANSI keyboard.
fn keycode(key: Key) -> Option<u16> {
    Some(match key {
        Key::Enter => 0x24,
        Key::Tab => 0x30,
        Key::Escape => 0x35,
        Key::Backspace => 0x33,
        Key::Delete => 0x75,
        Key::Space => 0x31,
        Key::Up => 0x7E,
        Key::Down => 0x7D,
        Key::Left => 0x7B,
        Key::Right => 0x7C,
        Key::Home => 0x73,
        Key::End => 0x77,
        Key::PageUp => 0x74,
        Key::PageDown => 0x79,
        Key::F(n) => *[
            0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F,
        ]
        .get(usize::from(n).checked_sub(1)?)?,
        Key::Char(c) => match c {
            'a' => 0x00,
            's' => 0x01,
            'd' => 0x02,
            'f' => 0x03,
            'h' => 0x04,
            'g' => 0x05,
            'z' => 0x06,
            'x' => 0x07,
            'c' => 0x08,
            'v' => 0x09,
            'b' => 0x0B,
            'q' => 0x0C,
            'w' => 0x0D,
            'e' => 0x0E,
            'r' => 0x0F,
            'y' => 0x10,
            't' => 0x11,
            '1' => 0x12,
            '2' => 0x13,
            '3' => 0x14,
            '4' => 0x15,
            '6' => 0x16,
            '5' => 0x17,
            '=' => 0x18,
            '9' => 0x19,
            '7' => 0x1A,
            '-' => 0x1B,
            '8' => 0x1C,
            '0' => 0x1D,
            ']' => 0x1E,
            'o' => 0x1F,
            'u' => 0x20,
            '[' => 0x21,
            'i' => 0x22,
            'p' => 0x23,
            'l' => 0x25,
            'j' => 0x26,
            '\'' => 0x27,
            'k' => 0x28,
            ';' => 0x29,
            '\\' => 0x2A,
            ',' => 0x2B,
            '/' => 0x2C,
            'n' => 0x2D,
            'm' => 0x2E,
            '.' => 0x2F,
            '`' => 0x32,
            _ => return None,
        },
    })
}
