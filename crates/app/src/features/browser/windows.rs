//! The in-app browser on Windows: WebView2 inside a borderless popup owned by
//! the app's window. GPUI composites its content with DirectComposition in
//! front of child windows, so a child web view would stay hidden behind it;
//! an owned popup sits above its owner instead, and is moved over the card
//! in screen coordinates (the card re-renders when the window moves). Windows
//! 11 rounds the popup's corners.

use std::ffi::c_void;
use std::num::NonZeroIsize;
use std::sync::Once;

use gentle_core::browser::Profile;
use gpui_kit::{Bounds, Pixels, Window};
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SWP_HIDEWINDOW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, WNDCLASSW, WS_CLIPCHILDREN,
    WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::w;

/// The Win32 loop GPUI already runs drives WebView2.
pub(super) const NEEDS_PUMP: bool = false;

pub(super) fn pump() {}

pub(super) struct NativeView {
    /// Dropped before the popup it lives in is destroyed.
    webview: Option<wry::WebView>,
    /// The profile's WebView2 user data folder; kept while the view lives.
    _context: Option<wry::WebContext>,
    popup: HWND,
    owner: HWND,
    /// Screen rect (physical px) it was last placed at, and whether it showed.
    placed: Option<(i32, i32, i32, i32, bool)>,
}

/// A window handle for `wry` to build the web view in.
struct Parent(HWND);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?;
        let raw = RawWindowHandle::Win32(Win32WindowHandle::new(hwnd));
        // SAFETY: the popup outlives every use `wry` makes of this handle.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

unsafe extern "system" fn popup_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

const CLASS: windows::core::PCWSTR = w!("GentleShellBrowserPopup");

fn register_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(popup_proc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);
    });
}

/// The app's window, which owns the popup.
pub(super) struct Host(HWND);

pub(super) fn host(window: &Window) -> Result<Host, String> {
    // GPUI's own `Window::window_handle` shadows the trait's.
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => Ok(Host(HWND(handle.hwnd.get() as *mut c_void))),
        _ => Err("The browser could not find the app's window.".into()),
    }
}

impl NativeView {
    /// Pumps the thread's messages until WebView2 is ready
    /// (`webview2_com::wait_with_pump`), so it must run outside any GPUI
    /// update: GPUI's window procedure re-enters and would panic.
    pub(super) fn new(
        host: Host,
        url: &str,
        background: (u8, u8, u8, u8),
        script: Option<&str>,
        sandbox: Option<&super::Sandbox>,
        profile: &Profile,
        relays: super::Relays,
    ) -> Result<Self, String> {
        let owner = host.0;
        register_class();
        let popup = unsafe {
            let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS,
                w!(""),
                WS_POPUP | WS_CLIPCHILDREN,
                0,
                0,
                1,
                1,
                Some(owner),
                None,
                Some(module.into()),
                None,
            )
            .map_err(|error| format!("The browser could not start: {error}"))?
        };
        unsafe {
            let corners = DWMWCP_ROUND;
            // Windows 10 has no rounded corners; the call just fails there.
            let _ = DwmSetWindowAttribute(
                popup,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corners as *const _ as *const c_void,
                std::mem::size_of_val(&corners) as u32,
            );
        }
        // Keep every persistent profile, including Shared, under app data.
        let mut context = (!profile.is_private())
            .then(|| super::profile_dir(profile).map(|dir| wry::WebContext::new(Some(dir))))
            .flatten();
        let builder = match context.as_mut() {
            Some(context) => wry::WebViewBuilder::new_with_web_context(context),
            None => wry::WebViewBuilder::new(),
        }
        .with_incognito(profile.is_private())
        .with_url(url)
        // Runs before the page's own scripts (a device panel names its device).
        .with_initialization_script(script.unwrap_or_default())
        .with_background_color(background)
        .with_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(0, 0).into(),
            size: wry::dpi::PhysicalSize::new(1u32, 1u32).into(),
        });
        let builder = super::relay(builder, relays);
        let webview = match sandbox {
            Some(sandbox) => super::isolate(builder, sandbox),
            None => builder,
        }
        .build_as_child(&Parent(popup));
        match webview {
            Ok(webview) => Ok(Self {
                webview: Some(webview),
                _context: context,
                popup,
                owner,
                placed: None,
            }),
            Err(error) => {
                unsafe {
                    let _ = DestroyWindow(popup);
                }
                Err(format!(
                    "The browser could not start: {error}. It needs the Microsoft Edge WebView2 Runtime."
                ))
            }
        }
    }

    pub(super) fn place(&mut self, window: &Window, bounds: Bounds<Pixels>, visible: bool) {
        let scale = window.scale_factor();
        let mut origin = POINT {
            x: (bounds.origin.x.as_f32() * scale).round() as i32,
            y: (bounds.origin.y.as_f32() * scale).round() as i32,
        };
        unsafe {
            let _ = ClientToScreen(self.owner, &mut origin);
        }
        let width = ((bounds.size.width.as_f32() * scale).round() as i32).max(1);
        let height = ((bounds.size.height.as_f32() * scale).round() as i32).max(1);
        let key = (origin.x, origin.y, width, height, visible);
        if self.placed == Some(key) {
            return;
        }
        let show = if visible {
            SWP_SHOWWINDOW
        } else {
            SWP_HIDEWINDOW
        };
        unsafe {
            let _ = SetWindowPos(
                self.popup,
                None,
                origin.x,
                origin.y,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER | show,
            );
        }
        if let Some(webview) = &self.webview {
            let _ = webview.set_bounds(wry::Rect {
                position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                size: wry::dpi::PhysicalSize::new(width as u32, height as u32).into(),
            });
        }
        self.placed = Some(key);
    }

    pub(super) fn load_url(&self, url: &str) {
        if let Some(webview) = &self.webview {
            let _ = webview.load_url(url);
        }
    }

    pub(super) fn reload(&self) {
        if let Some(webview) = &self.webview {
            let _ = webview.reload();
        }
    }

    pub(super) fn evaluate(&self, script: &str) {
        if let Some(webview) = &self.webview {
            let _ = webview.evaluate_script(script);
        }
    }

    /// The page's popup takes the keyboard from the app's window, so GPUI
    /// never asks while the page has it.
    pub(super) fn has_keyboard(&self) -> bool {
        false
    }

    /// Run `script`; `done` gets its value as JSON (nothing if it never ran).
    pub(super) fn evaluate_with(&self, script: &str, done: impl Fn(String) + Send + 'static) {
        if let Some(webview) = &self.webview {
            let _ = webview.evaluate_script_with_callback(script, done);
        }
    }

    /// The page as WebView2 draws it (CapturePreview), as PNG bytes.
    pub(super) fn screenshot(&self, done: super::Shot) {
        use webview2_com::CapturePreviewCompletedHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
        use windows::Win32::UI::Shell::SHCreateMemStream;
        use wry::WebViewExtWindows as _;

        let Some(webview) = &self.webview else {
            done(Err("The page isn't open.".into()));
            return;
        };
        let Some(stream) = (unsafe { SHCreateMemStream(None) }) else {
            done(Err("No memory for the picture.".into()));
            return;
        };
        let read = stream.clone();
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
            done(
                result
                    .map_err(|error| format!("WebView2 couldn't take the picture: {error}"))
                    .and_then(|()| read_stream(&read)),
            );
            Ok(())
        }));
        // SAFETY: the stream and handler live until WebView2 calls back.
        let started = unsafe {
            webview.webview().CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler,
            )
        };
        if let Err(error) = started {
            eprintln!("gentle-shell-gpui: CapturePreview failed: {error}");
        }
    }

    /// Forget the profile's cookies, storage and caches.
    pub(super) fn clear_data(&self) {
        if let Some(webview) = &self.webview {
            let _ = webview.clear_all_browsing_data();
        }
    }

    /// Hide the popup now; the next `place` shows it again.
    pub(super) fn conceal(&mut self) {
        unsafe {
            let _ = SetWindowPos(
                self.popup,
                None,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_HIDEWINDOW,
            );
        }
        self.placed = None;
    }
}

impl Drop for NativeView {
    fn drop(&mut self) {
        self.webview = None;
        unsafe {
            let _ = DestroyWindow(self.popup);
        }
    }
}

/// The page lives in its own native window here, which gives the keyboard
/// back when the app's window is clicked.
pub(super) fn reclaim_keyboard(_window: &Window) {}

/// Everything written to `stream`, from the start.
fn read_stream(stream: &windows::Win32::System::Com::IStream) -> Result<Vec<u8>, String> {
    use windows::Win32::System::Com::{STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};
    let mut stat = STATSTG::default();
    // SAFETY: a memory stream WebView2 finished writing to.
    unsafe {
        stream
            .Stat(&mut stat, STATFLAG_NONAME)
            .map_err(|error| error.to_string())?;
        stream
            .Seek(0, STREAM_SEEK_SET, None)
            .map_err(|error| error.to_string())?;
        let mut bytes = vec![0u8; stat.cbSize as usize];
        let mut got = 0u32;
        stream
            .Read(
                bytes.as_mut_ptr().cast(),
                bytes.len() as u32,
                Some(&mut got),
            )
            .ok()
            .map_err(|error| error.to_string())?;
        bytes.truncate(got as usize);
        Ok(bytes)
    }
}
