//! The launcher window (Windows): a small Win32 window, written against the system libraries directly so that the
//! program gains no dependency. What it says and what a click leads to is in `launcher`; here is only the drawing, the
//! message loop, the file dialog and the clipboard.
//!
//! The window lives on a thread of its own with its message loop. The build runs on another thread and reaches the
//! window with posted messages (a `Screen` to show); a question (the compiler download) waits for its answer on a
//! channel. The window gets out of the way when the game starts and comes back with the error if the game fails.

use crate::update;
use crate::launcher::{self, Button, Key, Lang, Screen};
use crate::port;
use crate::ui::{DownloadNotice, Ui};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};

type Hwnd = isize;

const CLASS: &str = "SnesPortLauncher";
const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_SETFONT: u32 = 0x0030;
const WM_COMMAND: u32 = 0x0111;
const WM_CTLCOLORSTATIC: u32 = 0x0138;
const WM_APP_SCREEN: u32 = 0x8001;
const WM_APP_HIDE: u32 = 0x8002;
const WM_APP_SHOW: u32 = 0x8003;
const WM_APP_QUIT: u32 = 0x8004;

const WS_CAPTION: u32 = 0x00C0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_MINIMIZEBOX: u32 = 0x0002_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_BORDER: u32 = 0x0080_0000;
const BS_DEFPUSHBUTTON: u32 = 1;
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_READONLY: u32 = 0x0800;
const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const COLOR_WINDOW: isize = 5;
const IDC_ARROW: usize = 32512;
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
const OFN_EXPLORER: u32 = 0x0008_0000;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;
const LOGPIXELSX: i32 = 88;
const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;
const ID_PRIMARY: i32 = 1;
const ID_SECONDARY: i32 = 2;
const ID_COPY: i32 = 3;
const ID_UPDATE: i32 = 4;
const ID_LANG: i32 = 5;
const ID_LICENCES: i32 = 6;
const ID_CHECK: i32 = 7;
const MB_OK: u32 = 0;
const MB_ICONERROR: u32 = 0x10;
const MB_ICONINFORMATION: u32 = 0x40;

const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_DRAWITEM: u32 = 0x002B;
const BS_OWNERDRAW: u32 = 0x0B;
const ODS_SELECTED: u32 = 0x0001;
const ODS_FOCUS: u32 = 0x0010;
const DT_CENTER: u32 = 0x0001;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;
const DT_END_ELLIPSIS: u32 = 0x8000;
const DT_NOPREFIX: u32 = 0x0800;

// The look, the same as the Android and Linux launchers (`ZeldaUi.java`, `linux/launcher.c`): dark green stone with a faint
// checkerboard, a bevelled gold frame with a diamond in each corner, the Triforce and plate buttons. The numbers are
// 0xRRGGBB and `rgb` turns them into the COLORREF (0x00BBGGRR) that GDI wants.
const OUTLINE: u32 = 0x2A1A06;
const GOLD_HI: u32 = 0xFFF0A0;
const GOLD: u32 = 0xF8D860;
const GOLD_LO: u32 = 0xC08820;
const STONE: u32 = 0x1C3A2A;
const STONE_HI: u32 = 0x2E5A40;
const PRESS: u32 = 0x48B070;
const SHADOW: u32 = 0x28180A;
const TEXT: u32 = 0xC8D8B8;
const TEXT_HI: u32 = 0xFFFCE8;
const BG_TOP: u32 = 0x0C1C14;
const BG_BOTTOM: u32 = 0x1A3828;
const ERROR_TITLE: u32 = 0xFF9A8A;
const DIM: u32 = 0x8FA890;
const CLIENT_W: i32 = 540;
const CLIENT_H: i32 = 340;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const DC_BRUSH: i32 = 18;
const SRCCOPY: u32 = 0x00CC_0020;
const DT_WORDBREAK: u32 = 0x0010;
const DT_TOP: u32 = 0;

fn rgb(c: u32) -> u32 {
    ((c & 0xFF) << 16) | (c & 0xFF00) | (c >> 16)
}

/// `a` towards `b` by `n` parts of `d`, channel by channel.
fn mix(a: u32, b: u32, n: i32, d: i32) -> u32 {
    let ch = |sh: u32| {
        let (x, y) = (((a >> sh) & 0xFF) as i32, ((b >> sh) & 0xFF) as i32);
        (x + (y - x) * n / d.max(1)) as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PaintStruct {
    hdc: isize,
    erase: i32,
    paint: Rect,
    restore: i32,
    update: i32,
    reserved: [u8; 32],
}

#[repr(C)]
struct DrawItem {
    ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    action: u32,
    state: u32,
    item: Hwnd,
    dc: isize,
    rc: Rect,
    data: usize,
}

#[repr(C)]
struct WndClassW {
    style: u32,
    proc_: unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize,
    cls_extra: i32,
    wnd_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct OpenFileNameW {
    size: u32,
    owner: Hwnd,
    instance: isize,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    def_ext: *const u16,
    cust_data: isize,
    hook: isize,
    template: *const u16,
    reserved: *mut c_void,
    reserved_dw: u32,
    flags_ex: u32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(c: *const WndClassW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, inst: isize, param: *mut c_void) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn GetMessageW(m: *mut Msg, h: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(m: *const Msg) -> i32;
    fn DispatchMessageW(m: *const Msg) -> isize;
    fn IsDialogMessageW(h: Hwnd, m: *mut Msg) -> i32;
    fn PostMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> i32;
    fn SendMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn PostQuitMessage(code: i32);
    fn ShowWindow(h: Hwnd, cmd: i32) -> i32;
    fn SetWindowTextW(h: Hwnd, text: *const u16) -> i32;
    fn DestroyWindow(h: Hwnd) -> i32;
    fn LoadCursorW(inst: isize, name: usize) -> isize;
    fn LoadIconW(inst: isize, name: usize) -> isize;
    fn AdjustWindowRect(r: *mut Rect, style: u32, menu: i32) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn SetForegroundWindow(h: Hwnd) -> i32;
    fn OpenClipboard(h: Hwnd) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(fmt: u32, h: isize) -> isize;
    fn CloseClipboard() -> i32;
    fn GetDC(h: Hwnd) -> isize;
    fn ReleaseDC(h: Hwnd, dc: isize) -> i32;
    fn InvalidateRect(h: Hwnd, r: *const Rect, erase: i32) -> i32;
    fn UpdateWindow(h: Hwnd) -> i32;
    fn BeginPaint(h: Hwnd, ps: *mut PaintStruct) -> isize;
    fn EndPaint(h: Hwnd, ps: *const PaintStruct) -> i32;
    fn FillRect(dc: isize, r: *const Rect, brush: isize) -> i32;
    fn DrawTextW(dc: isize, text: *const u16, n: i32, r: *mut Rect, format: u32) -> i32;
    fn GetWindowTextW(h: Hwnd, buf: *mut u16, n: i32) -> i32;
    fn MessageBoxW(h: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32, charset: u32, out: u32, clip: u32, quality: u32, pitch: u32, face: *const u16) -> isize;
    fn GetDeviceCaps(dc: isize, index: i32) -> i32;
    fn SetBkMode(dc: isize, mode: i32) -> i32;
    fn SetTextColor(dc: isize, color: u32) -> u32;
    fn CreateSolidBrush(color: u32) -> isize;
    fn SelectObject(dc: isize, obj: isize) -> isize;
    fn DeleteObject(obj: isize) -> i32;
    fn SetBkColor(dc: isize, color: u32) -> u32;
    fn SetDCBrushColor(dc: isize, color: u32) -> u32;
    fn GetStockObject(i: i32) -> isize;
    fn CreateCompatibleDC(dc: isize) -> isize;
    fn CreateCompatibleBitmap(dc: isize, w: i32, h: i32) -> isize;
    fn DeleteDC(dc: isize) -> i32;
    fn BitBlt(dst: isize, x: i32, y: i32, w: i32, h: i32, src: isize, sx: i32, sy: i32, rop: u32) -> i32;
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(o: *mut OpenFileNameW) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(h: isize) -> *mut c_void;
    fn GlobalUnlock(h: isize) -> i32;
    fn AttachConsole(pid: u32) -> i32;
    fn GetConsoleWindow() -> isize;
    fn GetStdHandle(n: u32) -> isize;
    fn SetStdHandle(n: u32, h: isize) -> i32;
    fn CreateFileW(name: *const u16, access: u32, share: u32, sec: *mut c_void, disp: u32, flags: u32, template: isize) -> isize;
    fn GetUserDefaultUILanguage() -> u16;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Edit controls want `\r\n`.
fn crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

// ---- the console of the terminal that started the program ----

/// A program of the windows subsystem has no console of its own. When it is started from a terminal it takes that
/// one (and its standard handles, unless the terminal redirected them), so that the messages and the exit code are
/// what they were. Returns true when there is a console to write to: false when started by a double click or a
/// drop of the ROM on the program.
pub fn attach_console() -> bool {
    unsafe {
        if GetConsoleWindow() == 0 && AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return false;
        }
        for (std_handle, file, access) in [(STD_INPUT_HANDLE, "CONIN$", 0x8000_0000u32), (STD_OUTPUT_HANDLE, "CONOUT$", 0x4000_0000), (STD_ERROR_HANDLE, "CONOUT$", 0x4000_0000)] {
            let h = GetStdHandle(std_handle);
            if h == 0 || h == -1 {
                // 3 = OPEN_EXISTING, 3 = FILE_SHARE_READ | FILE_SHARE_WRITE
                let f = CreateFileW(wide(file).as_ptr(), access, 3, std::ptr::null_mut(), 3, 0, 0);
                if f != -1 {
                    SetStdHandle(std_handle, f);
                }
            }
        }
        true
    }
}

pub fn ui_language() -> Lang {
    launcher::lang_from_langid(unsafe { GetUserDefaultUILanguage() })
}

// ---- state shared by the threads ----

struct Gui {
    /// Spanish (true) or English: the person can switch it with the language button.
    es: AtomicBool,
    cache: PathBuf,
    /// The window offers to choose a ROM (started without one), as opposed to a ROM given to the program.
    picks_rom: bool,
    hwnd: AtomicIsize,
    answer: Mutex<Option<mpsc::Sender<bool>>>,
    screen: Mutex<Screen>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

static GUI: OnceLock<Gui> = OnceLock::new();
static EXIT: AtomicI32 = AtomicI32::new(0);

fn lang() -> Lang {
    if gui().es.load(Ordering::SeqCst) {
        Lang::Es
    } else {
        Lang::En
    }
}

/// The name the window carries: `name=` of `update.info` (the package of a game says its own), else the engine's.
fn app_title() -> String {
    let from_file = std::env::current_exe().ok().and_then(|e| std::fs::read_to_string(e.parent()?.join("update.info")).ok()).and_then(|t| {
        t.lines().find_map(|l| l.strip_prefix("name=").map(|n| n.trim().to_string())).filter(|n| !n.is_empty())
    });
    from_file.unwrap_or_else(|| launcher::text(lang(), Key::WindowTitle).to_string())
}

/// The line under the title: `tagline=` of `update.info` (the name of the game), else the engine's, in the language of the window.
fn app_tagline() -> String {
    let from_file = std::env::current_exe().ok().and_then(|e| std::fs::read_to_string(e.parent()?.join("update.info")).ok()).and_then(|t| {
        t.lines().find_map(|l| l.strip_prefix("tagline=").map(|n| n.trim().to_string())).filter(|n| !n.is_empty())
    });
    from_file.unwrap_or_else(|| launcher::text(lang(), Key::Tagline).to_string())
}

/// What the line under the body says (the version, or the answer of the last check).
static STATUS: Mutex<String> = Mutex::new(String::new());

fn set_status(text: &str) {
    *STATUS.lock().unwrap() = text.to_string();
    let h = gui().hwnd.load(Ordering::SeqCst);
    if h != 0 {
        unsafe { InvalidateRect(h, std::ptr::null(), 0) };
    }
}

fn gui() -> &'static Gui {
    GUI.get().expect("launcher window not initialised")
}

struct Controls {
    detail: Hwnd,
    primary: Hwnd,
    secondary: Hwnd,
    copy: Hwnd,
    /// The check box of the update notice (only when this installation updates itself).
    update: Hwnd,
    lang: Hwnd,
    licences: Hwnd,
    check: Hwnd,
}

/// The state of that check box (it is drawn by hand, so it does not keep its own).
static UPDATE_ON: AtomicBool = AtomicBool::new(false);
/// The text of the body, painted by the window itself so that it sits on the stone without leaving traces.
static BODY: Mutex<String> = Mutex::new(String::new());

static CONTROLS: OnceLock<Controls> = OnceLock::new();

/// The fonts, brush and icon the window paints with (made once, when it is created).
struct Look {
    title: isize,
    sub: isize,
    body: isize,
    button: isize,
    /// The dark stone behind the box with the detail of an error.
    dark: isize,
}

static LOOK: OnceLock<Look> = OnceLock::new();

fn dpi() -> i32 {
    unsafe {
        let dc = GetDC(0);
        let d = if dc != 0 { GetDeviceCaps(dc, LOGPIXELSX) } else { 96 };
        if dc != 0 {
            ReleaseDC(0, dc);
        }
        d.max(96)
    }
}

fn scale(v: i32) -> i32 {
    v * dpi() / 96
}

fn font(points: i32, weight: i32) -> isize {
    unsafe { CreateFontW(-(points * dpi() / 72), 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr()) }
}

// ---- the window ----

unsafe fn child(parent: Hwnd, class: &str, text: &str, style: u32, id: i32, x: i32, y: i32, w: i32, h: i32, f: isize) -> Hwnd {
    let inst = GetModuleHandleW(std::ptr::null());
    let c = CreateWindowExW(0, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | style, scale(x), scale(y), scale(w), scale(h), parent, id as isize, inst, std::ptr::null_mut());
    SendMessageW(c, WM_SETFONT, f as usize, 1);
    c
}

unsafe fn create_controls(hwnd: Hwnd) {
    let normal = font(10, 400);
    let _ = LOOK.set(Look {
        title: font(17, 700),
        sub: font(10, 400),
        body: normal,
        button: font(10, 700),
        dark: CreateSolidBrush(rgb(BG_TOP)),
    });
    let detail = child(hwnd, "EDIT", "", WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_TABSTOP, 0, 32, 128, 476, 148, normal);
    let copy = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW, ID_COPY, 36, 286, 156, 34, normal);
    let secondary = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW, ID_SECONDARY, 360, 286, 144, 34, normal);
    let primary = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW | BS_DEFPUSHBUTTON, ID_PRIMARY, 198, 286, 156, 34, normal);
    let update = child(hwnd, "BUTTON", launcher::text(lang(), Key::UpdateCheckbox), WS_TABSTOP | BS_OWNERDRAW, ID_UPDATE, 36, 254, 350, 22, normal);
    let lang_btn = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW, ID_LANG, 452, 24, 52, 24, normal);
    let licences = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW, ID_LICENCES, 36, 286, 120, 34, normal);
    let check = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW, ID_CHECK, 394, 252, 110, 26, normal);
    if update_info().is_some() {
        UPDATE_ON.store(update::load_settings(&gui().cache).check, Ordering::SeqCst);
    }
    let _ = CONTROLS.set(Controls { lang: lang_btn, licences, check, detail, primary, secondary, copy, update });
}

fn set_text(h: Hwnd, s: &str) {
    unsafe { SetWindowTextW(h, wide(s).as_ptr()) };
}

fn show_control(h: Hwnd, visible: bool) {
    unsafe { ShowWindow(h, if visible { SW_SHOW } else { SW_HIDE }) };
}

/// The names of the games the profiles at hand know, for the window to say what it can play.
fn supported_games() -> String {
    match crate::games::catalog(&[]) {
        Ok(c) => c.profiles().iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", "),
        Err(_) => String::new(),
    }
}

/// Draws a screen: the texts, the buttons that apply and which of them is the main one.
fn draw(screen: &Screen) {
    let g = gui();
    let c = CONTROLS.get().expect("controls");
    let t = |k: Key| launcher::text(lang(), k);
    let games = supported_games();
    let (body, detail, primary, secondary, third): (String, Option<String>, Option<&str>, Option<&str>, Option<&str>) = match screen {
        Screen::Pick => (
            if games.is_empty() { t(Key::PickPrompt).to_string() } else { format!("{games}\n\n{}", t(Key::PickPrompt)) },
            None,
            Some(t(Key::PickAction)),
            None,
            None,
        ),
        Screen::Ready(rom) => (format!("{}\n\n{}", t(Key::PlayWith), launcher::rom_label(rom)), None, Some(t(Key::PlayAction)), Some(t(Key::ChangeAction)), None),
        Screen::Busy(text) => (text.clone(), None, None, None, None),
        Screen::Ask(text) => (text.clone(), None, Some(t(Key::DownloadYes)), Some(t(Key::DownloadNo)), None),
        Screen::Failed { detail, .. } => (
            String::new(),
            Some(detail.clone()),
            if g.picks_rom { Some(t(Key::ChooseAnother)) } else { None },
            Some(t(Key::Close)),
            Some(t(Key::CopyDetail)),
        ),
        Screen::Update { text, .. } => (text.clone(), None, Some(t(Key::UpdateInstall)), Some(t(Key::UpdateLater)), Some(t(Key::UpdateSkip))),
        Screen::Licences(text) => (String::new(), Some(text.clone()), None, Some(t(Key::Close)), None),
    };
    *BODY.lock().unwrap() = body;
    show_control(c.detail, detail.is_some());
    if let Some(d) = &detail {
        set_text(c.detail, &crlf(d));
    }
    for (h, label) in [(c.primary, primary), (c.secondary, secondary), (c.copy, third)] {
        show_control(h, label.is_some());
        if let Some(l) = label {
            set_text(h, l);
        }
    }
    let home = matches!(screen, Screen::Pick | Screen::Ready(_));
    let updates = update_info().is_some() && home;
    show_control(c.update, updates);
    show_control(c.check, updates);
    show_control(c.licences, home);
    show_control(c.lang, home);
    set_text(c.update, t(Key::UpdateCheckbox));
    set_text(c.check, t(Key::CheckNow));
    set_text(c.licences, t(Key::Licences));
    set_text(c.lang, t(Key::LangButton));
    *g.screen.lock().unwrap() = screen.clone();
    // what was hidden or shown is painted now, not at the next occasion
    let h = g.hwnd.load(Ordering::SeqCst);
    if h != 0 {
        unsafe {
            InvalidateRect(h, std::ptr::null(), 1);
            UpdateWindow(h);
        }
    }
}

fn post_screen(screen: Screen) {
    let h = gui().hwnd.load(Ordering::SeqCst);
    let raw = Box::into_raw(Box::new(screen));
    if h == 0 || unsafe { PostMessageW(h, WM_APP_SCREEN, 0, raw as isize) } == 0 {
        drop(unsafe { Box::from_raw(raw) });
    }
}

fn post(msg: u32) {
    let h = gui().hwnd.load(Ordering::SeqCst);
    if h != 0 {
        unsafe { PostMessageW(h, msg, 0, 0) };
    }
}

fn copy_to_clipboard(owner: Hwnd, text: &str) -> bool {
    unsafe {
        if OpenClipboard(owner) == 0 {
            return false;
        }
        EmptyClipboard();
        let data = wide(&crlf(text));
        let bytes = data.len() * 2;
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        let mut ok = false;
        if mem != 0 {
            let p = GlobalLock(mem) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                GlobalUnlock(mem);
                ok = SetClipboardData(CF_UNICODETEXT, mem) != 0;
            }
        }
        CloseClipboard();
        ok
    }
}

/// The ROM dialog; `None` when the person closes it without choosing.
fn pick_rom(owner: Hwnd) -> Option<PathBuf> {
    let g = gui();
    let start = port::start_dir(&g.cache);
    let mut file = vec![0u16; 32768];
    let filter: Vec<u16> = launcher::text(lang(), Key::PickFilter).encode_utf16().collect();
    let title = wide(launcher::text(lang(), Key::PickTitle));
    let dir = wide(&start.to_string_lossy());
    let mut o = OpenFileNameW {
        size: std::mem::size_of::<OpenFileNameW>() as u32,
        owner,
        instance: 0,
        filter: filter.as_ptr(),
        custom_filter: std::ptr::null_mut(),
        max_custom: 0,
        filter_index: 1,
        file: file.as_mut_ptr(),
        max_file: file.len() as u32,
        file_title: std::ptr::null_mut(),
        max_file_title: 0,
        initial_dir: if start.as_os_str().is_empty() { std::ptr::null() } else { dir.as_ptr() },
        title: title.as_ptr(),
        flags: OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER,
        file_offset: 0,
        file_extension: 0,
        def_ext: std::ptr::null(),
        cust_data: 0,
        hook: 0,
        template: std::ptr::null(),
        reserved: std::ptr::null_mut(),
        reserved_dw: 0,
        flags_ex: 0,
    };
    if unsafe { GetOpenFileNameW(&mut o) } == 0 {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..len])))
}

/// Starts the build and the game for `rom` on a thread of its own.
fn start(rom: PathBuf) {
    draw(&Screen::Busy(launcher::text(lang(), Key::Working).to_string()));
    std::thread::spawn(move || {
        let ui = WindowUi::new();
        let code = port::run_port(rom, gui().cache.clone(), true, false, false, Vec::new(), Vec::new(), &ui);
        finish(code, &ui);
    });
}

/// What follows the end of a run: leave when it went well, else show why it did not.
fn finish(code: i32, ui: &WindowUi) {
    if code == 0 {
        EXIT.store(0, Ordering::SeqCst);
        post(WM_APP_QUIT);
        return;
    }
    let screen = if ui.launched.load(Ordering::SeqCst) { launcher::game_failed(lang(), code) } else { launcher::failed(lang(), code, &ui.errors.lock().unwrap().join("\n")) };
    EXIT.store(code, Ordering::SeqCst);
    post_screen(screen);
    post(WM_APP_SHOW);
}

unsafe fn on_click(hwnd: Hwnd, id: i32) {
    if id == ID_LANG {
        let new = launcher::other_lang(lang());
        gui().es.store(new == Lang::Es, Ordering::SeqCst);
        launcher::save_lang(&gui().cache, new);
        STATUS.lock().unwrap().clear();
        SetWindowTextW(hwnd, wide(&app_title()).as_ptr());
        let screen = gui().screen.lock().unwrap().clone();
        draw(&screen);
        return;
    }
    if id == ID_LICENCES {
        let dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)).unwrap_or_default();
        draw(&Screen::Licences(launcher::licence_text(lang(), &dir)));
        return;
    }
    if id == ID_CHECK {
        if let Some(info) = update_info() {
            std::thread::spawn(move || update_flow(info, true));
        }
        return;
    }
    if id == ID_UPDATE {
        if let (Some(c), Some(info)) = (CONTROLS.get(), update_info()) {
            let on = !UPDATE_ON.fetch_xor(true, Ordering::SeqCst);
            InvalidateRect(c.update, std::ptr::null(), 1);
            update::set_check(&gui().cache, on);
            if on {
                std::thread::spawn(move || update_flow(info, true));
            }
        }
        return;
    }
    let button = match id {
        ID_PRIMARY => Button::Primary,
        ID_SECONDARY => Button::Secondary,
        ID_COPY => Button::Copy,
        _ => return,
    };
    let screen = gui().screen.lock().unwrap().clone();
    match (&screen, button) {
        (Screen::Pick, Button::Primary) | (Screen::Ready(_), Button::Secondary) | (Screen::Failed { .. }, Button::Primary) => {
            if let launcher::Next::Start(rom) = launcher::after_dialog(pick_rom(hwnd)) {
                start(rom);
            }
        }
        (Screen::Ready(rom), Button::Primary) => start(rom.clone()),
        (Screen::Ask(_), b) => {
            if let Some(tx) = gui().answer.lock().unwrap().take() {
                let _ = tx.send(b == Button::Primary);
            }
            draw(&Screen::Busy(launcher::text(lang(), Key::Working).to_string()));
        }
        (Screen::Failed { .. }, Button::Secondary) => {
            DestroyWindow(hwnd);
        }
        (Screen::Update { .. }, Button::Primary) => {
            if let Some(info) = update_info() {
                std::thread::spawn(move || install_now(&info));
            }
        }
        (Screen::Update { .. }, Button::Secondary) | (Screen::Licences(_), Button::Secondary) => draw(&launcher::first_screen(&gui().cache)),
        (Screen::Update { tag, .. }, Button::Copy) => {
            update::skip(&gui().cache, tag);
            draw(&launcher::first_screen(&gui().cache));
        }
        (Screen::Failed { detail, .. }, Button::Copy) => {
            if copy_to_clipboard(hwnd, detail) {
                if let Some(c) = CONTROLS.get() {
                    set_text(c.copy, launcher::text(lang(), Key::Copied));
                }
            }
        }
        _ => {}
    }
}

/// What the header says for a screen: the title, the line under it and whether it is an error.
fn header_texts(lang: Lang, screen: &Screen) -> (String, String, bool) {
    match screen {
        Screen::Failed { title, .. } => (title.clone(), app_title(), true),
        Screen::Update { .. } => (launcher::text(lang, Key::UpdateTitle).to_string(), app_tagline(), false),
        Screen::Licences(_) => (launcher::text(lang, Key::Licences).to_string(), app_tagline(), false),
        _ => (app_title(), app_tagline(), false),
    }
}

unsafe fn text_at(dc: isize, text: &str, font: isize, color: u32, rect: &mut Rect, format: u32) {
    let old = SelectObject(dc, font);
    SetTextColor(dc, color);
    let w = wide(text);
    DrawTextW(dc, w.as_ptr(), -1, rect, format | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS);
    SelectObject(dc, old);
}

/// A filled rectangle in layout pixels (scaled to the screen).
unsafe fn fr(dc: isize, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let r = Rect { left: scale(x), top: scale(y), right: scale(x + w), bottom: scale(y + h) };
    SetDCBrushColor(dc, rgb(color));
    FillRect(dc, &r, GetStockObject(DC_BRUSH));
}

/// One piece of the Triforce: a triangle with a gold bevel, light from the top left; `w` wide at the base, `h` high.
unsafe fn triangle(dc: isize, cx: i32, top: i32, w: i32, h: i32) {
    for y in 0..h {
        let half = y * (w / 2) / h + 1;
        let shade = |x: i32| {
            if y >= h - 3 || x >= half - 1 {
                if y == h - 1 { OUTLINE } else { GOLD_LO }
            } else if y == h - 1 || x == half {
                GOLD_LO
            } else if x == -half || y < 2 {
                GOLD_HI
            } else {
                GOLD
            }
        };
        // the row in three parts: the left edge, the body and the right edge
        fr(dc, cx - half, top + y, 1, 1, shade(-half));
        if half > 0 {
            fr(dc, cx - half + 1, top + y, 2 * half - 1, 1, shade(0));
        }
        fr(dc, cx + half, top + y, 1, 1, shade(half));
    }
    fr(dc, cx - w / 2 - 1, top + h, w + 3, 1, OUTLINE);
}

/// The stone: a green gradient with a faint checkerboard, three gold filets and a diamond in each corner.
unsafe fn backdrop(dc: isize) {
    for y in (0..CLIENT_H).step_by(4) {
        let base = mix(BG_TOP, BG_BOTTOM, y, CLIENT_H);
        fr(dc, 0, y, CLIENT_W, 4, base);
        let lit = mix(base, STONE_HI, 1, 10);
        for cell in 0..=(CLIENT_W / 16) {
            if (cell + y / 16) % 2 == 0 {
                fr(dc, cell * 16, y, 16, 4, lit);
            }
        }
    }
    for (i, col) in [OUTLINE, GOLD, GOLD_LO].into_iter().enumerate() {
        let m = 8 + i as i32 * 2;
        fr(dc, m, m, CLIENT_W - 2 * m, 2, col);
        fr(dc, m, CLIENT_H - m - 2, CLIENT_W - 2 * m, 2, col);
        fr(dc, m, m, 2, CLIENT_H - 2 * m, col);
        fr(dc, CLIENT_W - m - 2, m, 2, CLIENT_H - 2 * m, col);
    }
    for i in 0..4 {
        let cx = if i & 1 != 0 { CLIENT_W - 20 } else { 20 };
        let cy = if i & 2 != 0 { CLIENT_H - 20 } else { 20 };
        for d in 0..7 {
            fr(dc, cx - (6 - d), cy - d, 2 * (6 - d) + 1, 1, if d < 2 { GOLD_HI } else { GOLD });
            fr(dc, cx - (6 - d), cy + d, 2 * (6 - d) + 1, 1, if d < 2 { GOLD } else { GOLD_LO });
        }
    }
    triangle(dc, CLIENT_W / 2, 20, 26, 22);
    triangle(dc, CLIENT_W / 2 - 14, 44, 26, 22);
    triangle(dc, CLIENT_W / 2 + 14, 44, 26, 22);
}

/// Centred text with a dark shadow one pixel down and to the right.
unsafe fn shadowed(dc: isize, text: &str, font: isize, color: u32, x: i32, y: i32, w: i32, h: i32) {
    let mut r = Rect { left: scale(x + 1), top: scale(y + 1), right: scale(x + w + 1), bottom: scale(y + h + 1) };
    text_at(dc, text, font, rgb(SHADOW), &mut r, DT_CENTER);
    let mut r = Rect { left: scale(x), top: scale(y), right: scale(x + w), bottom: scale(y + h) };
    text_at(dc, text, font, rgb(color), &mut r, DT_CENTER);
}

/// The whole window, painted off screen and copied in one go: the stone, the title, the line under it and the body.
unsafe fn paint(hwnd: Hwnd) {
    let mut ps = std::mem::zeroed::<PaintStruct>();
    let screen_dc = BeginPaint(hwnd, &mut ps);
    if let Some(look) = LOOK.get() {
        let (w, h) = (scale(CLIENT_W), scale(CLIENT_H));
        let dc = CreateCompatibleDC(screen_dc);
        let bmp = CreateCompatibleBitmap(screen_dc, w, h);
        let old = SelectObject(dc, bmp);
        backdrop(dc);
        let g = gui();
        let screen = g.screen.lock().unwrap().clone();
        let (title, sub, error) = header_texts(lang(), &screen);
        SetBkMode(dc, 1);
        shadowed(dc, &title, look.title, if error { ERROR_TITLE } else { GOLD_HI }, 24, 72, CLIENT_W - 48, 28);
        shadowed(dc, &sub, look.sub, TEXT, 24, 100, CLIENT_W - 48, 20);
        if !matches!(screen, Screen::Failed { .. } | Screen::Licences(_)) {
            let body = BODY.lock().unwrap().clone();
            let mut r = Rect { left: scale(32), top: scale(132), right: scale(CLIENT_W - 32), bottom: scale(250) };
            let old_font = SelectObject(dc, look.body);
            SetTextColor(dc, rgb(TEXT));
            let text = wide(&body);
            DrawTextW(dc, text.as_ptr(), -1, &mut r, DT_CENTER | DT_TOP | DT_WORDBREAK | DT_NOPREFIX);
            SelectObject(dc, old_font);
        }
        if matches!(screen, Screen::Pick | Screen::Ready(_)) {
            if let Some(info) = update_info() {
                let status = STATUS.lock().unwrap().clone();
                let line = if status.is_empty() { launcher::text(lang(), Key::VersionLabel).replace("{v}", &info.version).replace("{e}", crate::ENGINE_COMMIT) } else { status };
                let mut r = Rect { left: scale(32), top: scale(228), right: scale(CLIENT_W - 32), bottom: scale(248) };
                text_at(dc, &line, look.sub, rgb(DIM), &mut r, DT_CENTER);
            }
        }
        BitBlt(screen_dc, 0, 0, w, h, dc, 0, 0, SRCCOPY);
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
    }
    EndPaint(hwnd, &ps);
}

/// A rectangle of device pixels inside `rc`, in one colour.
unsafe fn fill_in(dc: isize, rc: &Rect, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let r = Rect { left: rc.left + x, top: rc.top + y, right: rc.left + x + w, bottom: rc.top + y + h };
    SetDCBrushColor(dc, rgb(color));
    FillRect(dc, &r, GetStockObject(DC_BRUSH));
}

/// A plate: dark green stone inside a bevelled gold border (lighter and greener while pressed), the label on it. The main
/// button has a brighter label; the one with the focus gets a light inner line.
unsafe fn plate(dc: isize, rc: &Rect, label: &str, font: isize, main: bool, pressed: bool, focus: bool) {
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    let t = scale(2).max(2);
    let (hi, mid, lo) = if pressed { (GOLD, GOLD_LO, GOLD_LO) } else { (GOLD_HI, GOLD, GOLD_LO) };
    fill_in(dc, rc, 0, 0, w, h, OUTLINE);
    fill_in(dc, rc, 1, 1, w - 2, h - 2, mid);
    fill_in(dc, rc, 1, 1, w - 2, t, hi);
    fill_in(dc, rc, 1, 1, t, h - 2, hi);
    fill_in(dc, rc, 1, h - 1 - t, w - 2, t, lo);
    fill_in(dc, rc, w - 1 - t, 1, t, h - 2, lo);
    let (x0, y0) = (1 + t, 1 + t);
    let (iw, ih) = (w - 2 * (1 + t), h - 2 * (1 + t));
    fill_in(dc, rc, x0, y0, iw, ih, if pressed { PRESS } else { STONE });
    if !pressed {
        fill_in(dc, rc, x0, y0, iw, 1, STONE_HI);
    }
    if focus {
        let f = scale(3);
        fill_in(dc, rc, x0 + f, y0 + f, iw - 2 * f, 1, GOLD_HI);
        fill_in(dc, rc, x0 + f, y0 + ih - f - 1, iw - 2 * f, 1, GOLD_HI);
        fill_in(dc, rc, x0 + f, y0 + f, 1, ih - 2 * f, GOLD_HI);
        fill_in(dc, rc, x0 + iw - f - 1, y0 + f, 1, ih - 2 * f, GOLD_HI);
    }
    let mut r = Rect { left: rc.left, top: rc.top, right: rc.right, bottom: rc.bottom };
    SetBkMode(dc, 1);
    text_at(dc, label, font, rgb(if main { TEXT_HI } else { GOLD_HI }), &mut r, DT_CENTER | DT_VCENTER);
}

/// The buttons, drawn by hand: plates, and the check box of the update notice as a small plate with a tick.
unsafe fn draw_button(di: &DrawItem) {
    let Some(look) = LOOK.get() else { return };
    let rc = Rect { left: di.rc.left, top: di.rc.top, right: di.rc.right, bottom: di.rc.bottom };
    let mut label = [0u16; 128];
    let n = GetWindowTextW(di.item, label.as_mut_ptr(), label.len() as i32).max(0) as usize;
    let text = String::from_utf16_lossy(&label[..n]);
    let pressed = di.state & ODS_SELECTED != 0;
    let focus = di.state & ODS_FOCUS != 0;
    if di.ctl_id == ID_UPDATE as u32 {
        // the stone behind it (the gradient there, without the checkerboard, which is too faint to matter)
        SetDCBrushColor(di.dc, rgb(mix(BG_TOP, BG_BOTTOM, 265, CLIENT_H)));
        FillRect(di.dc, &rc, GetStockObject(DC_BRUSH));
        let size = scale(14);
        let top = (rc.bottom - rc.top - size) / 2;
        let on = UPDATE_ON.load(Ordering::SeqCst);
        fill_in(di.dc, &rc, 0, top, size, size, GOLD_LO);
        fill_in(di.dc, &rc, 1, top + 1, size - 2, size - 2, if on { PRESS } else { STONE });
        if on {
            let u = (size / 14).max(1);
            for (x, y, w, h) in [(4, 9, 2, 4), (6, 11, 2, 2), (8, 6, 2, 6), (10, 4, 2, 4)] {
                fill_in(di.dc, &rc, x * u, top + y * u, w * u, h * u, TEXT_HI);
            }
        }
        let mut r = Rect { left: rc.left + size + scale(8), top: rc.top, right: rc.right, bottom: rc.bottom };
        SetBkMode(di.dc, 1);
        text_at(di.dc, &text, look.body, rgb(if focus { GOLD_HI } else { TEXT }), &mut r, DT_VCENTER);
        return;
    }
    plate(di.dc, &rc, &text, look.button, di.ctl_id == ID_PRIMARY as u32, pressed, focus);
}

unsafe extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        WM_CREATE => {
            create_controls(hwnd);
            0
        }
        WM_COMMAND => {
            on_click(hwnd, (wp & 0xFFFF) as i32);
            0
        }
        WM_ERASEBKGND => 1, // `paint` covers everything, and an erase first would flicker
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_DRAWITEM => {
            draw_button(&*(lp as *const DrawItem));
            1
        }
        WM_CTLCOLORSTATIC => {
            // the box with the detail of an error: light text on dark stone
            SetBkColor(wp as isize, rgb(BG_TOP));
            SetTextColor(wp as isize, rgb(TEXT));
            LOOK.get().map_or(0, |l| l.dark)
        }
        WM_APP_SCREEN => {
            let screen = Box::from_raw(lp as *mut Screen);
            draw(&screen);
            0
        }
        WM_APP_HIDE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_APP_SHOW => {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            0
        }
        WM_APP_QUIT => {
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// The thread of the window: creates it showing `first`, reports its handle and runs the message loop until it closes.
fn window_thread(first: Screen, ready: mpsc::Sender<()>) {
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide(CLASS);
        let wc = WndClassW {
            style: 0,
            proc_: wndproc,
            cls_extra: 0,
            wnd_extra: 0,
            instance: inst,
            icon: LoadIconW(inst, 1), // the icon of the program (resource 1, see build.rs)
            cursor: LoadCursorW(0, IDC_ARROW),
            background: COLOR_WINDOW + 1,
            menu: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
        let mut r = Rect { left: 0, top: 0, right: scale(CLIENT_W), bottom: scale(CLIENT_H) };
        AdjustWindowRect(&mut r, style, 0);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let (x, y) = ((GetSystemMetrics(0) - w) / 2, (GetSystemMetrics(1) - h) / 3);
        let title = wide(&app_title());
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), style, x, y, w, h, 0, 0, inst, std::ptr::null_mut());
        if hwnd == 0 {
            let _ = ready.send(());
            return;
        }
        gui().hwnd.store(hwnd, Ordering::SeqCst);
        draw(&first);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        let _ = ready.send(());
        let mut msg = std::mem::zeroed::<Msg>();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    // the window is gone: so is the program, with the code the last run left
    std::process::exit(EXIT.load(Ordering::SeqCst));
}

fn open_window(first: Screen) {
    let g = gui();
    let mut slot = g.thread.lock().unwrap();
    if slot.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    *slot = Some(std::thread::spawn(move || window_thread(first, tx)));
    let _ = rx.recv();
}

// ---- the `Ui` of the window ----

/// Tells the build's messages to the window. The window opens when something first has to be said, so a ROM that is
/// already built starts without one.
pub struct WindowUi {
    launched: AtomicBool,
    errors: Mutex<Vec<String>>,
}

impl WindowUi {
    fn new() -> Self {
        WindowUi { launched: AtomicBool::new(false), errors: Mutex::new(Vec::new()) }
    }

    fn busy(&self, text: String) {
        open_window(Screen::Busy(text.clone()));
        post_screen(Screen::Busy(text));
    }

    fn ask(&self, text: String) -> bool {
        open_window(Screen::Busy(String::new()));
        let (tx, rx) = mpsc::channel();
        *gui().answer.lock().unwrap() = Some(tx);
        post_screen(Screen::Ask(text));
        rx.recv().unwrap_or(false)
    }
}

impl Ui for WindowUi {
    fn status(&self, text: &str) {
        // the line that announces the first build is the one worth saying (in the language of the window)
        let shown = if text.starts_with("Building ") { launcher::text(lang(), Key::FirstBuild).to_string() } else { text.to_string() };
        self.busy(shown);
    }

    fn error(&self, text: &str) {
        self.errors.lock().unwrap().push(text.to_string());
    }

    fn confirm(&self, question: &str) -> bool {
        self.ask(question.to_string())
    }

    fn offer_download(&self, notice: &DownloadNotice) -> bool {
        let text = match lang() {
            Lang::En => format!(
                "No C compiler was found on this machine. To build the game, {} ({} KiB) can be downloaded from {} and kept in {}.\n\nNothing about your ROM is sent; it is the only download.",
                notice.name,
                notice.kib,
                notice.url,
                launcher::shorten_path(&notice.dir.display().to_string(), 48)
            ),
            Lang::Es => format!(
                "No se ha encontrado ningún compilador de C en este equipo. Para construir el juego se puede descargar {} ({} KiB) de {} y guardarlo en {}.\n\nNo se envía nada de tu ROM; es la única descarga.",
                notice.name,
                notice.kib,
                notice.url,
                launcher::shorten_path(&notice.dir.display().to_string(), 48)
            ),
        };
        self.ask(text)
    }

    fn ask_rom(&self, _start_dir: &Path) -> Option<PathBuf> {
        None
    }

    fn launching(&self) {
        self.launched.store(true, Ordering::SeqCst);
        post(WM_APP_HIDE);
    }
}

// ---- update notice ----

/// What this installation says about itself (`update.info` next to the executable); None: it does not update itself.
fn update_info() -> Option<update::Info> {
    update::read_info(std::env::current_exe().ok()?.parent()?)
}

fn message(text: &str, caption: &str, kind: u32) -> i32 {
    let h = gui().hwnd.load(Ordering::SeqCst);
    unsafe { MessageBoxW(h, wide(text).as_ptr(), wide(caption).as_ptr(), kind) }
}

/// The screen that offers a new version.
fn update_screen(info: &update::Info, tag: &str) -> Screen {
    let text = launcher::text(lang(), Key::UpdateAsk).replace("{tag}", tag).replace("{current}", &info.version);
    Screen::Update { tag: tag.to_string(), text }
}

/// The check and what follows, in a thread of its own because it waits for the network. `manual`: the person asked for it
/// (the box just ticked, or the button), so every answer is told, a skipped version is offered again and the answer shows
/// under the body; otherwise only a new version is told, and only while the window is waiting for a choice.
fn update_flow(info: update::Info, manual: bool) {
    let g = gui();
    let t = |k: Key| launcher::text(lang(), k);
    for _ in 0..40 {
        if g.hwnd.load(Ordering::SeqCst) != 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if manual {
        set_status(t(Key::Checking));
    }
    let waiting = || matches!(&*g.screen.lock().unwrap(), Screen::Pick | Screen::Ready(_));
    match update::check(&g.cache, &info, manual) {
        update::Outcome::Available { tag, .. } if waiting() => {
            set_status("");
            post_screen(update_screen(&info, &tag));
        }
        update::Outcome::Skipped if manual && waiting() => {
            set_status("");
            let tag = update::load_settings(&g.cache).tag;
            post_screen(update_screen(&info, &tag));
        }
        update::Outcome::UpToDate if manual => set_status(t(Key::UpdateUpToDate)),
        update::Outcome::Failed(_) if manual => set_status(t(Key::UpdateUnreachable)),
        _ if manual => set_status(""),
        _ => {}
    }
}

fn install_now(info: &update::Info) {
    let g = gui();
    let t = |k: Key| launcher::text(lang(), k);
    post_screen(Screen::Busy(t(Key::Updating).to_string()));
    let Some(exe) = std::env::current_exe().ok() else { return };
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    match update::install(info, &dir, &g.cache.join("update-work")) {
        Ok(_) => {
            message(t(Key::UpdateDone), t(Key::UpdateTitle), MB_OK | MB_ICONINFORMATION);
            // an installation from before the package carried the name of the game still runs `snes-port.exe`: the update
            // brings `alttp-recomp.exe`, which is what starts now, and the old one is set aside (it is deleted at the next start)
            let renamed = std::fs::read_dir(&dir).ok().and_then(|rd| {
                rd.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "exe") && p != &exe && p.file_stem().is_some_and(|s| s != "snes-port" && s != "snes-recomp"))
            });
            let next = match renamed {
                Some(new) if exe.file_stem().is_some_and(|s| s == "snes-port") => {
                    let _ = std::fs::rename(&exe, exe.with_extension("exe.old"));
                    new
                }
                _ => exe.clone(),
            };
            let _ = std::process::Command::new(&next).spawn(); // the new executable
            EXIT.store(0, Ordering::SeqCst);
            post(WM_APP_QUIT);
        }
        Err(e) => {
            post_screen(launcher::first_screen(&g.cache));
            message(&format!("{} {e}", t(Key::UpdateFailed)), t(Key::UpdateTitle), MB_OK | MB_ICONERROR);
        }
    }
}

// ---- entry points ----

fn init(cache: &Path, picks_rom: bool) {
    let _ = GUI.set(Gui {
        es: AtomicBool::new(launcher::load_lang(cache).unwrap_or_else(ui_language) == Lang::Es),
        cache: cache.to_path_buf(),
        picks_rom,
        hwnd: AtomicIsize::new(0),
        answer: Mutex::new(None),
        screen: Mutex::new(Screen::Pick),
        thread: Mutex::new(None),
    });
}

/// The launcher: the window opens at once and everything follows from its buttons. Returns when the window closes.
pub fn run_launcher(cache: &Path) -> i32 {
    init(cache, true);
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        update::clean_old(&dir); // leftovers of a previous update
    }
    open_window(launcher::first_screen(cache));
    if let Some(info) = update_info() {
        if update::load_settings(cache).check {
            std::thread::spawn(move || update_flow(info, false));
        }
    }
    let handle = gui().thread.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.join();
    }
    EXIT.load(Ordering::SeqCst)
}

/// A ROM given to the program with no terminal to talk to (dropped on the icon): builds and starts it, and shows a
/// window only when there is something to say (a build, a question, an error).
pub fn run_rom(rom: PathBuf, cache: &Path, launch: bool, fast: bool, download_yes: bool, game_args: Vec<String>, profiles: Vec<PathBuf>) -> i32 {
    init(cache, false);
    let ui = WindowUi::new();
    let code = port::run_port(rom, cache.to_path_buf(), launch, fast, download_yes, game_args, profiles, &ui);
    let had_window = gui().thread.lock().unwrap().is_some();
    if code == 0 {
        if had_window {
            EXIT.store(0, Ordering::SeqCst);
            post(WM_APP_QUIT);
        }
        return 0;
    }
    // a failure always gets its window, and the program waits for it to be read
    let screen = if ui.launched.load(Ordering::SeqCst) { launcher::game_failed(lang(), code) } else { launcher::failed(lang(), code, &ui.errors.lock().unwrap().join("\n")) };
    EXIT.store(code, Ordering::SeqCst);
    open_window(screen.clone());
    post_screen(screen);
    post(WM_APP_SHOW);
    let handle = gui().thread.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.join();
    }
    code
}
