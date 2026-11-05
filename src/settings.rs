//! Janela de configurações (aberta pelo menu da bandeja): atalho, cor, espessura, tamanho
//! do texto, qualidade JPEG e "iniciar com o Windows". Salvar grava settings.json,
//! registra o novo atalho na hora e ajusta a inicialização automática.

use crate::config::{Color, HotkeyChoice, Settings};
use crate::{hotkey, startup};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetDlgItem,
    GetWindowTextW, HMENU, IDC_ARROW, IsDialogMessageW, LoadCursorW, MB_ICONINFORMATION,
    MB_OK, MSG, MessageBoxW, RegisterClassW, SYSTEM_PARAMETERS_INFO_ACTION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW, SW_SHOW, SendMessageW,
    SetForegroundWindow, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_DESTROY, WM_SETFONT, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("ScreenshotterSettings");

const ID_HOTKEY: i32 = 101;
const ID_COLOR: i32 = 102;
const ID_STROKE: i32 = 103;
const ID_TEXT: i32 = 104;
const ID_QUALITY: i32 = 105;
const ID_STARTUP: i32 = 106;
const ID_SAVE: i32 = 1; // IDOK: Enter aciona
const ID_CANCEL: i32 = 2; // IDCANCEL: Esc aciona

// Estilos de controles (valores do SDK).
const BS_PUSHBUTTON: u32 = 0;
const BS_DEFPUSHBUTTON: u32 = 1;
const BS_AUTOCHECKBOX: u32 = 3;
const CBS_DROPDOWNLIST: u32 = 3;
const ES_AUTOHSCROLL: u32 = 0x80;
const ES_NUMBER: u32 = 0x2000;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;

/// Janela principal (dona do atalho global): precisa ser reregistrada ao salvar.
static MAIN: AtomicIsize = AtomicIsize::new(0);
/// Janela de configurações aberta (0 = nenhuma), para IsDialogMessage no loop.
static OPEN: AtomicIsize = AtomicIsize::new(0);

const HOTKEYS: [(HotkeyChoice, &str); 3] = [
    (HotkeyChoice::ShiftBackspace, "Shift + Backspace"),
    (HotkeyChoice::PrtScn, "PrtScn"),
    (HotkeyChoice::ShiftPrtScn, "Shift + PrtScn"),
];

const COLORS: [(Color, &str); 6] = [
    (Color::Yellow, "Amarelo"),
    (Color::Blue, "Azul"),
    (Color::Green, "Verde"),
    (Color::Red, "Vermelho"),
    (Color::Black, "Preto"),
    (Color::White, "Branco"),
];

fn send(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { SendMessageW(h, msg, Some(wp), Some(lp)) }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Repassa teclas (Tab, Enter, Esc) à janela de configurações. Verdadeiro se tratou a mensagem.
pub fn handle_dialog_message(msg: &MSG) -> bool {
    let h = OPEN.load(Ordering::Relaxed);
    if h == 0 {
        return false;
    }
    unsafe { IsDialogMessageW(HWND(h as *mut _), msg).as_bool() }
}

fn child(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    style: u32,
    ex: u32,
    id: i32,
    (x, y, w, h): (i32, i32, i32, i32),
    s: f32,
    font: usize,
) -> HWND {
    let sc = |v: i32| (v as f32 * s).round() as i32;
    let t = wide(text);
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let c = CreateWindowExW(
            WINDOW_EX_STYLE(ex),
            class,
            PCWSTR(t.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
            sc(x),
            sc(y),
            sc(w),
            sc(h),
            Some(parent),
            Some(HMENU(id as usize as *mut _)),
            Some(hinst.into()),
            None,
        )
        .unwrap_or_default();
        send(c, WM_SETFONT, WPARAM(font), LPARAM(1));
        c
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            match (wp.0 & 0xFFFF) as i32 {
                ID_SAVE => {
                    save(hwnd);
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                ID_CANCEL => unsafe {
                    let _ = DestroyWindow(hwnd);
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            OPEN.store(0, Ordering::Relaxed);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn num(hwnd: HWND, id: i32, lo: u32, hi: u32, fallback: u32) -> u32 {
    let mut buf = [0u16; 16];
    let n = unsafe {
        let c = GetDlgItem(Some(hwnd), id).unwrap_or_default();
        GetWindowTextW(c, &mut buf)
    } as usize;
    String::from_utf16_lossy(&buf[..n]).trim().parse::<u32>().map(|v| v.clamp(lo, hi)).unwrap_or(fallback)
}

fn save(hwnd: HWND) {
    let path = crate::settings_path();
    let mut s = Settings::load(&path);
    unsafe {
        let item = |id| GetDlgItem(Some(hwnd), id).unwrap_or_default();
        let hk = send(item(ID_HOTKEY), CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if let Some((c, _)) = HOTKEYS.get(hk as usize) {
            s.hotkey = *c;
        }
        let co = send(item(ID_COLOR), CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if let Some((c, _)) = COLORS.get(co as usize) {
            s.default_color = *c;
        }
        s.start_with_windows = send(item(ID_STARTUP), BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;
    }
    s.stroke_width = num(hwnd, ID_STROKE, 1, 20, s.stroke_width);
    s.text_size = num(hwnd, ID_TEXT, 8, 96, s.text_size);
    s.jpeg_quality = num(hwnd, ID_QUALITY, 1, 100, s.jpeg_quality as u32) as u8;

    if s.save(&path).is_err() {
        info(hwnd, "Não foi possível gravar as configurações.");
        return;
    }
    let want = s.start_with_windows;
    std::thread::spawn(move || {
        let _ = startup::set_enabled(want);
    });
    let main = HWND(MAIN.load(Ordering::Relaxed) as *mut _);
    let (_, warn) = hotkey::register(main, s.hotkey);
    if let Some(w) = warn {
        info(hwnd, w);
    }
}

fn info(hwnd: HWND, text: &str) {
    let t = wide(text);
    unsafe {
        MessageBoxW(Some(hwnd), PCWSTR(t.as_ptr()), w!("Screenshotter"), MB_OK | MB_ICONINFORMATION);
    }
}

/// Abre a janela (ou traz a existente para frente).
pub fn open(main: HWND) {
    MAIN.store(main.0 as isize, Ordering::Relaxed);
    unsafe {
        if let Ok(existing) = FindWindowW(CLASS, None) {
            let _ = SetForegroundWindow(existing);
            return;
        }
        let cur = {
            let p = crate::settings_path();
            let mut s = Settings::load(&p);
            // Sem arquivo ainda: o estado real de "iniciar com o Windows" vem do registro.
            if !p.exists() {
                s.start_with_windows = startup::is_enabled();
            }
            s
        };

        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(16 as *mut _), // COLOR_BTNFACE + 1
            ..Default::default()
        };
        RegisterClassW(&wc);

        let s = (GetDpiForSystem() as f32 / 96.0).max(1.0);
        let sc = |v: i32| (v as f32 * s).round() as i32;
        let style = WS_CAPTION | WS_SYSMENU;
        let mut r = RECT { left: 0, top: 0, right: sc(330), bottom: sc(218) };
        let _ = AdjustWindowRectEx(&mut r, style, false, WINDOW_EX_STYLE(0));
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        // Centralizada na área de trabalho (acima da barra de tarefas), como no Lightshot.
        let mut wa = RECT::default();
        let _ = SystemParametersInfoW(
            SYSTEM_PARAMETERS_INFO_ACTION(48), // SPI_GETWORKAREA
            0,
            Some(&mut wa as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let x = wa.left + (wa.right - wa.left - ww) / 2;
        let y = wa.top + (wa.bottom - wa.top - wh) / 2;

        let title = wide("Screenshotter — Configurações");
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            PCWSTR(title.as_ptr()),
            style,
            x,
            y,
            ww,
            wh,
            None,
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };

        let font = CreateFontW(
            -(9.5 * s * 96.0 / 72.0).round() as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            windows::Win32::Graphics::Gdi::FONT_CHARSET(1),
            windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0),
            windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),
            windows::Win32::Graphics::Gdi::FONT_QUALITY(0),
            0,
            w!("Segoe UI"),
        );
        let f = font.0 as usize;
        let tab = WS_TABSTOP.0;
        let label = |text: &str, y: i32| {
            child(hwnd, w!("STATIC"), text, 0, 0, 0, (12, y + 3, 150, 18), s, f);
        };

        label("Atalho de captura", 12);
        let hk = child(hwnd, w!("COMBOBOX"), "", CBS_DROPDOWNLIST | tab, 0, ID_HOTKEY, (170, 12, 148, 200), s, f);
        for (_, n) in HOTKEYS {
            let t = wide(n);
            send(hk, CB_ADDSTRING, WPARAM(0), LPARAM(t.as_ptr() as isize));
        }
        let idx = HOTKEYS.iter().position(|(c, _)| *c == cur.hotkey).unwrap_or(0);
        send(hk, CB_SETCURSEL, WPARAM(idx), LPARAM(0));

        label("Cor padrão", 40);
        let co = child(hwnd, w!("COMBOBOX"), "", CBS_DROPDOWNLIST | tab, 0, ID_COLOR, (170, 40, 148, 200), s, f);
        for (_, n) in COLORS {
            let t = wide(n);
            send(co, CB_ADDSTRING, WPARAM(0), LPARAM(t.as_ptr() as isize));
        }
        let idx = COLORS.iter().position(|(c, _)| *c == cur.default_color).unwrap_or(3);
        send(co, CB_SETCURSEL, WPARAM(idx), LPARAM(0));

        let edit_style = ES_NUMBER | ES_AUTOHSCROLL | tab | WS_BORDER.0;
        label("Espessura (1–20)", 68);
        child(hwnd, w!("EDIT"), &cur.stroke_width.to_string(), edit_style, 0, ID_STROKE, (170, 68, 60, 22), s, f);
        label("Texto (8–96)", 96);
        child(hwnd, w!("EDIT"), &cur.text_size.to_string(), edit_style, 0, ID_TEXT, (170, 96, 60, 22), s, f);
        label("Qualidade JPEG (1–100)", 124);
        child(hwnd, w!("EDIT"), &cur.jpeg_quality.to_string(), edit_style, 0, ID_QUALITY, (170, 124, 60, 22), s, f);

        let cb = child(hwnd, w!("BUTTON"), "Iniciar com o Windows", BS_AUTOCHECKBOX | tab, 0, ID_STARTUP, (12, 158, 306, 20), s, f);
        send(cb, BM_SETCHECK, WPARAM(cur.start_with_windows as usize), LPARAM(0));

        child(hwnd, w!("BUTTON"), "Salvar", BS_DEFPUSHBUTTON | tab, 0, ID_SAVE, (158, 184, 76, 26), s, f);
        child(hwnd, w!("BUTTON"), "Cancelar", BS_PUSHBUTTON | tab, 0, ID_CANCEL, (242, 184, 76, 26), s, f);

        if let Ok(icon) = crate::tray::load_tray_icon() {
            send(hwnd, 0x80, WPARAM(0), LPARAM(icon.0 as isize)); // WM_SETICON, ICON_SMALL
        }
        OPEN.store(hwnd.0 as isize, Ordering::Relaxed);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}
