//! Janela de configurações (aberta pelo menu da bandeja): atalho, cor, espessura e tamanho
//! do texto (barras deslizantes com prévia ao lado) e "iniciar com o Windows". A qualidade
//! JPEG não é exposta aqui: fica no valor do settings.json (padrão 80). Salvar grava settings.json,
//! registra o novo atalho na hora e ajusta a inicialização automática.

use crate::config::{Color, HotkeyChoice, Settings};
use crate::{hotkey, startup};
use crate::gfx::{Canvas, argb_from_colorref};
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FillRect, HBRUSH, HGDIOBJ, InvalidateRect, PAINTSTRUCT, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetDlgItem,
    HMENU, IDC_ARROW, IsDialogMessageW, LoadCursorW, MB_ICONINFORMATION,
    MB_OK, MSG, MessageBoxW, RegisterClassW, SYSTEM_PARAMETERS_INFO_ACTION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW, SW_SHOW, SendMessageW,
    SetForegroundWindow, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_DESTROY, WM_PAINT, WM_SETFONT, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("ScreenshotterSettings");

const ID_HOTKEY: i32 = 101;
const ID_COLOR: i32 = 102;
const ID_STARTUP: i32 = 106;
const ID_SAVE: i32 = 1; // IDOK: Enter aciona
const ID_CANCEL: i32 = 2; // IDCANCEL: Esc aciona

// Estilos de controles (valores do SDK).
const BS_PUSHBUTTON: u32 = 0;
const BS_DEFPUSHBUTTON: u32 = 1;
const BS_AUTOCHECKBOX: u32 = 3;
const CBS_DROPDOWNLIST: u32 = 3;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const CBN_SELCHANGE: usize = 1;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_MOUSEMOVE: u32 = 0x0200;

// Faixas das barras (espessura do traço e tamanho do texto, em pixels).
const STROKE_RANGE: (u32, u32) = (1, 20);
const TEXT_RANGE: (u32, u32) = (10, 36);
/// Barras desenhadas à mão: (centro y, x inicial, x final) em pixels lógicos.
const TRACK_X0: f32 = 176.0;
const TRACK_X1: f32 = 280.0;
const TRACK_Y: [f32; 2] = [90.0, 142.0];
/// Caixas de prévia (x, y, largura, altura), alinhadas à borda direita das listas.
const PREVIEW: [(i32, i32, i32, i32); 2] = [(298, 68, 90, 44), (298, 120, 90, 44)];

/// Valores atuais das barras: espessura e tamanho do texto.
static VALS: [AtomicU32; 2] = [AtomicU32::new(3), AtomicU32::new(18)];
/// Barra sendo arrastada (0, 1) ou 9 se nenhuma.
static DRAG: AtomicU32 = AtomicU32::new(9);

const RANGES: [(u32, u32); 2] = [STROKE_RANGE, TEXT_RANGE];

/// Janela principal (dona do atalho global): precisa ser reregistrada ao salvar.
static MAIN: AtomicIsize = AtomicIsize::new(0);
/// Janela de configurações aberta (0 = nenhuma), para IsDialogMessage no loop.
static OPEN: AtomicIsize = AtomicIsize::new(0);
/// Escala de DPI (bits de f32), para desenhar as prévias.
static SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

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
        WM_PAINT => {
            paint_previews(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = mouse_xy(lp);
            if let Some(i) = (0..2).find(|&i| {
                x >= TRACK_X0 - 10.0 && x <= TRACK_X1 + 10.0 && (y - TRACK_Y[i]).abs() <= 12.0
            }) {
                DRAG.store(i as u32, Ordering::Relaxed);
                unsafe {
                    SetCapture(hwnd);
                }
                set_from_x(hwnd, i, x);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let d = DRAG.load(Ordering::Relaxed) as usize;
            if d < 2 {
                let (x, _) = mouse_xy(lp);
                set_from_x(hwnd, d, x);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if DRAG.swap(9, Ordering::Relaxed) < 2 {
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if (wp.0 & 0xFFFF) as i32 == ID_COLOR && (wp.0 >> 16) & 0xFFFF == CBN_SELCHANGE {
                refresh_rows(hwnd);
            }
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

fn scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

fn mouse_xy(lp: LPARAM) -> (f32, f32) {
    let s = scale();
    let x = (lp.0 & 0xFFFF) as i16 as f32;
    let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
    (x / s, y / s)
}

/// Valor da barra `i` para a posição x (limitado à faixa).
fn value_at(x: f32, (lo, hi): (u32, u32)) -> u32 {
    let f = ((x - TRACK_X0) / (TRACK_X1 - TRACK_X0)).clamp(0.0, 1.0);
    lo + (f * (hi - lo) as f32).round() as u32
}

fn set_from_x(hwnd: HWND, i: usize, x: f32) {
    let v = value_at(x, RANGES[i]);
    if VALS[i].swap(v, Ordering::Relaxed) != v {
        refresh_rows(hwnd);
    }
}

fn slider(i: usize) -> u32 {
    VALS[i].load(Ordering::Relaxed).clamp(RANGES[i].0, RANGES[i].1)
}

fn current_color(hwnd: HWND) -> Color {
    let c = unsafe { GetDlgItem(Some(hwnd), ID_COLOR).unwrap_or_default() };
    let i = send(c, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0.max(0) as usize;
    COLORS.get(i).map(|(c, _)| *c).unwrap_or(Color::Red)
}

fn px_rect(r: (i32, i32, i32, i32), s: f32) -> RECT {
    let sc = |v: i32| (v as f32 * s).round() as i32;
    RECT { left: sc(r.0), top: sc(r.1), right: sc(r.0 + r.2), bottom: sc(r.1 + r.3) }
}

/// Faixa horizontal das duas linhas de barras + prévias (lado direito da janela).
fn rows_rect(s: f32) -> RECT {
    px_rect((166, 62, 230, 108), s)
}

fn refresh_rows(hwnd: HWND) {
    let rc = rows_rect(scale());
    unsafe {
        let _ = InvalidateRect(Some(hwnd), Some(&rc), false);
    }
}

/// Barras deslizantes (como um controle de volume) e prévias: um ponto com a espessura
/// do traço e "ABC" com o tamanho do texto, na cor escolhida.
fn paint_previews(hwnd: HWND) {
    let s = scale();
    let color = current_color(hwnd);
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        // Faixa das barras: fundo cinza do Windows antes de redesenhar.
        let area = rows_rect(s);
        FillRect(hdc, &area, HBRUSH(16 as *mut _)); // COLOR_BTNFACE + 1
        let argb = |rgb: u32| 0xFF00_0000 | rgb;
        if let Some(cv) = Canvas::from_hdc(hdc) {
            for i in 0..2 {
                let (lo, hi) = RANGES[i];
                let f = (slider(i) - lo) as f32 / (hi - lo) as f32;
                let cy = TRACK_Y[i];
                let len = TRACK_X1 - TRACK_X0;
                let sc = |r: (f32, f32, f32, f32)| (r.0 * s, r.1 * s, r.2 * s, r.3 * s);
                cv.fill_round(sc((TRACK_X0, cy - 2.0, len, 4.0)), 2.0 * s, argb(0xBBBBC2));
                if f > 0.0 {
                    cv.fill_round(sc((TRACK_X0, cy - 2.0, len * f, 4.0)), 2.0 * s, argb(0x6366F1));
                }
                let kx = TRACK_X0 + len * f;
                cv.fill_ellipse(sc((kx - 8.0, cy - 8.0, 16.0, 16.0)), argb(0x6366F1));
                cv.fill_ellipse(sc((kx - 3.0, cy - 3.0, 6.0, 6.0)), argb(0xFFFFFF));
                // Caixa de prévia branca de cantos arredondados.
                let pr = PREVIEW[i];
                let b = sc((pr.0 as f32, pr.1 as f32, pr.2 as f32, pr.3 as f32));
                cv.fill_round(b, 6.0 * s, argb(0xFFFFFF));
                cv.stroke_round(b, 6.0 * s, argb(0xC4C4CC), 1.0);
            }
            // Ponto: diâmetro = espessura do traço (contorno fino para o branco aparecer).
            let pr = PREVIEW[0];
            let (cx, cy) = ((pr.0 as f32 + pr.2 as f32 / 2.0) * s, (pr.1 as f32 + pr.3 as f32 / 2.0) * s);
            let d = slider(0) as f32 * s;
            let e = (cx - d / 2.0, cy - d / 2.0, d, d);
            cv.fill_ellipse(e, argb_from_colorref(color.colorref()));
            cv.stroke_ellipse(e, argb(0xB0B0B8), 1.0);
        }

        // Texto: "ABC" na altura configurada.
        let mut rc = px_rect(PREVIEW[1], s);
        let font = CreateFontW(
            -(slider(1) as f32 * s).round() as i32,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            windows::Win32::Graphics::Gdi::FONT_CHARSET(1),
            windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0),
            windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),
            windows::Win32::Graphics::Gdi::FONT_QUALITY(5),
            0,
            w!("Segoe UI"),
        );
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(color.colorref()));
        let mut t: Vec<u16> = "ABC".encode_utf16().collect();
        DrawTextW(hdc, &mut t, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(hdc, old);
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = EndPaint(hwnd, &ps);
    }
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
    s.stroke_width = slider(0);
    s.text_size = slider(1);

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
        SCALE.store(s.to_bits(), Ordering::Relaxed);
        VALS[0].store(cur.stroke_width.clamp(STROKE_RANGE.0, STROKE_RANGE.1), Ordering::Relaxed);
        VALS[1].store(cur.text_size.clamp(TEXT_RANGE.0, TEXT_RANGE.1), Ordering::Relaxed);
        DRAG.store(9, Ordering::Relaxed);
        let sc = |v: i32| (v as f32 * s).round() as i32;
        let style = WS_CAPTION | WS_SYSMENU;
        let mut r = RECT { left: 0, top: 0, right: sc(400), bottom: sc(240) };
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
        let hk = child(hwnd, w!("COMBOBOX"), "", CBS_DROPDOWNLIST | tab, 0, ID_HOTKEY, (170, 12, 218, 200), s, f);
        for (_, n) in HOTKEYS {
            let t = wide(n);
            send(hk, CB_ADDSTRING, WPARAM(0), LPARAM(t.as_ptr() as isize));
        }
        let idx = HOTKEYS.iter().position(|(c, _)| *c == cur.hotkey).unwrap_or(0);
        send(hk, CB_SETCURSEL, WPARAM(idx), LPARAM(0));

        label("Cor padrão", 40);
        let co = child(hwnd, w!("COMBOBOX"), "", CBS_DROPDOWNLIST | tab, 0, ID_COLOR, (170, 40, 218, 200), s, f);
        for (_, n) in COLORS {
            let t = wide(n);
            send(co, CB_ADDSTRING, WPARAM(0), LPARAM(t.as_ptr() as isize));
        }
        let idx = COLORS.iter().position(|(c, _)| *c == cur.default_color).unwrap_or(3);
        send(co, CB_SETCURSEL, WPARAM(idx), LPARAM(0));

        label("Espessura do traço", 80);
        label("Tamanho do texto", 132);

        let cb = child(hwnd, w!("BUTTON"), "Iniciar com o Windows", BS_AUTOCHECKBOX | tab, 0, ID_STARTUP, (12, 180, 376, 20), s, f);
        send(cb, BM_SETCHECK, WPARAM(cur.start_with_windows as usize), LPARAM(0));

        child(hwnd, w!("BUTTON"), "Salvar", BS_DEFPUSHBUTTON | tab, 0, ID_SAVE, (232, 208, 76, 26), s, f);
        child(hwnd, w!("BUTTON"), "Cancelar", BS_PUSHBUTTON | tab, 0, ID_CANCEL, (312, 208, 76, 26), s, f);

        if let Ok(icon) = crate::tray::load_tray_icon() {
            send(hwnd, 0x80, WPARAM(0), LPARAM(icon.0 as isize)); // WM_SETICON, ICON_SMALL
        }
        OPEN.store(hwnd.0 as isize, Ordering::Relaxed);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}
