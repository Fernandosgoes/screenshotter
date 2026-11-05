//! Janela de configurações (menu da bandeja). Desenhada à mão com GDI+ em três cartões:
//! Atalho (botões lado a lado), Desenho (cor, espessura, tamanho do texto) e Arquivo
//! (qualidade JPEG, iniciar com o Windows). Salvar grava settings.json, registra o novo
//! atalho na hora e ajusta a inicialização automática.

use crate::config::{Color, HotkeyChoice, Settings};
use crate::gfx::{Canvas, argb, argb_from_colorref};
use crate::{hotkey, startup};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, DRAW_TEXT_FORMAT, DT_CENTER,
    DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint,
    FONT_CHARSET, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, HDC, HFONT, HGDIOBJ, InvalidateRect,
    PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetSystemMetrics, IDC_ARROW,
    LoadCursorW, MB_ICONINFORMATION, MB_OK, MessageBoxW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    SendMessageW, SetForegroundWindow, ShowWindow, WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WNDCLASSW, WS_CAPTION, WS_SYSMENU,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("ScreenshotterSettings");

/// Tamanho da área útil em pixels lógicos (96 dpi).
const W: i32 = 400;
const H: i32 = 520;

// Cores (RGB), as mesmas do mockup: claro, com o índigo da barra de ferramentas.
const BG: u32 = 0x0D111B;
const FOOT: u32 = 0x121827;
const ACCENT_FG: u32 = 0x10131F;
const CARD: u32 = 0x161C2B;
const LINE: u32 = 0x273049;
const FIELD: u32 = 0x1D2538;
const INK: u32 = 0xE2E8F0;
const MUTED: u32 = 0x8B95AD;
const ACCENT: u32 = 0x818CF8;

type R = (f32, f32, f32, f32); // x, y, largura, altura

const CARD_HOTKEY: R = (14.0, 14.0, 372.0, 76.0);
const CARD_DRAW: R = (14.0, 102.0, 372.0, 214.0);
const CARD_FILE: R = (14.0, 328.0, 372.0, 124.0);

const SEG: R = (28.0, 48.0, 344.0, 30.0);
/// Largura de cada botão do atalho (proporcional ao texto); somam a largura de SEG.
const SEG_W: [f32; 3] = [140.0, 92.0, 112.0];

/// (x, largura) do botão i do atalho.
fn seg_bounds(i: usize) -> (f32, f32) {
    (SEG.0 + SEG_W[..i].iter().sum::<f32>(), SEG_W[i])
}
const SWATCH_Y: f32 = 169.0;
const SWATCH_X0: f32 = 41.0;
const SWATCH_STEP: f32 = 54.0;
const SWATCH_R: f32 = 13.0;
const TRACK_X0: f32 = 28.0;
const TRACK_X1: f32 = 320.0;
/// (centro y da trilha, mínimo, máximo): espessura, tamanho do texto, qualidade.
const SLIDERS: [(f32, u32, u32); 3] = [(232.0, 1, 20), (282.0, 8, 96), (394.0, 1, 100)];
const TOGGLE_ROW: R = (28.0, 414.0, 344.0, 26.0);
const BTN_SAVE: R = (296.0, 470.0, 90.0, 34.0);
const BTN_CANCEL: R = (198.0, 470.0, 90.0, 34.0);

const HOTKEYS: [(HotkeyChoice, &str); 3] = [
    (HotkeyChoice::ShiftBackspace, "Shift+Backspace"),
    (HotkeyChoice::PrtScn, "PrtScn"),
    (HotkeyChoice::ShiftPrtScn, "Shift+PrtScn"),
];

const COLORS: [Color; 6] = Color::ALL;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Seg(usize),
    Swatch(usize),
    Slider(usize),
    Toggle,
    Save,
    Cancel,
}

#[derive(Clone, Copy)]
struct State {
    hotkey: usize,
    color: usize,
    vals: [u32; 3], // espessura, tamanho do texto, qualidade
    startup: bool,
    drag: Option<usize>,
    scale: f32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Janela principal (dona do atalho global): o atalho é registrado de novo ao salvar.
static MAIN: AtomicIsize = AtomicIsize::new(0);

fn inside(r: R, x: f32, y: f32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

/// O que há sob o ponto (em pixels lógicos).
fn hit(x: f32, y: f32) -> Option<Hit> {
    if inside(SEG, x, y) {
        let i = (0..3).find(|&i| x < seg_bounds(i).0 + SEG_W[i]).unwrap_or(2);
        return Some(Hit::Seg(i));
    }
    for i in 0..COLORS.len() {
        let cx = SWATCH_X0 + SWATCH_STEP * i as f32;
        if (x - cx).hypot(y - SWATCH_Y) <= SWATCH_R + 4.0 {
            return Some(Hit::Swatch(i));
        }
    }
    for (i, (cy, _, _)) in SLIDERS.iter().enumerate() {
        if x >= TRACK_X0 - 10.0 && x <= TRACK_X1 + 10.0 && (y - cy).abs() <= 12.0 {
            return Some(Hit::Slider(i));
        }
    }
    if inside(TOGGLE_ROW, x, y) {
        return Some(Hit::Toggle);
    }
    if inside(BTN_SAVE, x, y) {
        return Some(Hit::Save);
    }
    if inside(BTN_CANCEL, x, y) {
        return Some(Hit::Cancel);
    }
    None
}

/// Valor da barra para uma posição x (limitado ao intervalo).
fn slider_value(x: f32, lo: u32, hi: u32) -> u32 {
    let f = ((x - TRACK_X0) / (TRACK_X1 - TRACK_X0)).clamp(0.0, 1.0);
    lo + (f * (hi - lo) as f32).round() as u32
}

fn slider_frac(v: u32, lo: u32, hi: u32) -> f32 {
    (v.clamp(lo, hi) - lo) as f32 / (hi - lo) as f32
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn colorref(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF))
}

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.lock().ok().and_then(|mut g| g.as_mut().map(f))
}

fn mouse(lp: LPARAM, s: f32) -> (f32, f32) {
    let x = (lp.0 & 0xFFFF) as i16 as f32;
    let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
    (x / s, y / s)
}

fn redraw(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn close(hwnd: HWND) {
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let s = with_state(|st| st.scale).unwrap_or(1.0);
            let (x, y) = mouse(lp, s);
            match hit(x, y) {
                Some(Hit::Seg(i)) => {
                    with_state(|st| st.hotkey = i);
                }
                Some(Hit::Swatch(i)) => {
                    with_state(|st| st.color = i);
                }
                Some(Hit::Slider(i)) => {
                    let (_, lo, hi) = SLIDERS[i];
                    with_state(|st| {
                        st.vals[i] = slider_value(x, lo, hi);
                        st.drag = Some(i);
                    });
                    unsafe {
                        SetCapture(hwnd);
                    }
                }
                Some(Hit::Toggle) => {
                    with_state(|st| st.startup = !st.startup);
                }
                Some(Hit::Save) => {
                    save(hwnd);
                    close(hwnd);
                    return LRESULT(0);
                }
                Some(Hit::Cancel) => {
                    close(hwnd);
                    return LRESULT(0);
                }
                None => {}
            }
            redraw(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let s = with_state(|st| st.scale).unwrap_or(1.0);
            let (x, _) = mouse(lp, s);
            let changed = with_state(|st| match st.drag {
                Some(i) => {
                    let (_, lo, hi) = SLIDERS[i];
                    st.vals[i] = slider_value(x, lo, hi);
                    true
                }
                None => false,
            });
            if changed == Some(true) {
                redraw(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            with_state(|st| st.drag = None);
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wp.0 as u16;
            if vk == VK_RETURN.0 {
                save(hwnd);
                close(hwnd);
            } else if vk == VK_ESCAPE.0 {
                close(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            close(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Ok(mut g) = STATE.lock() {
                *g = None;
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn font(s: f32, px: f32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -(px * s).round() as i32,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            FONT_CHARSET(1),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(5), // ClearType
            0,
            w!("Segoe UI"),
        )
    }
}

fn txt(dc: HDC, s: f32, text: &str, r: R, rgb: u32, align: DRAW_TEXT_FORMAT) {
    let mut t: Vec<u16> = text.encode_utf16().collect();
    let mut rc = RECT {
        left: (r.0 * s) as i32,
        top: (r.1 * s) as i32,
        right: ((r.0 + r.2) * s) as i32,
        bottom: ((r.1 + r.3) * s) as i32,
    };
    unsafe {
        SetTextColor(dc, colorref(rgb));
        DrawTextW(dc, &mut t, &mut rc, align | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
    }
}

fn shapes(dc: HDC, st: &State, pw: f32, ph: f32) {
    let s = st.scale;
    let sc = |r: R| (r.0 * s, r.1 * s, r.2 * s, r.3 * s);
    let rgb = |c: u32| argb(0xFF, c);
    let Some(cv) = Canvas::from_hdc(dc) else { return };
    cv.fill_round((0.0, 0.0, pw, ph), 0.0, rgb(BG));
    cv.fill_round((0.0, 458.0 * s, pw, ph - 458.0 * s), 0.0, rgb(FOOT));
    cv.line((0.0, 458.0 * s), (pw, 458.0 * s), rgb(LINE), 1.0);
    for c in [CARD_HOTKEY, CARD_DRAW, CARD_FILE] {
        cv.fill_round(sc(c), 12.0 * s, rgb(CARD));
        cv.stroke_round(sc(c), 12.0 * s, rgb(LINE), 1.0);
    }
    // Botões lado a lado do atalho.
    cv.fill_round(sc(SEG), 9.0 * s, rgb(FIELD));
    cv.stroke_round(sc(SEG), 9.0 * s, rgb(LINE), 1.0);
    let (bx, bw) = seg_bounds(st.hotkey);
    let on = (bx + 3.0, SEG.1 + 3.0, bw - 6.0, SEG.3 - 6.0);
    cv.fill_round(sc(on), 7.0 * s, rgb(ACCENT));
    // Bolinhas de cor.
    for (i, c) in COLORS.iter().enumerate() {
        let cx = SWATCH_X0 + SWATCH_STEP * i as f32;
        let d = SWATCH_R * 2.0 * s;
        let e = ((cx - SWATCH_R) * s, (SWATCH_Y - SWATCH_R) * s, d, d);
        cv.fill_ellipse(e, argb_from_colorref(c.colorref()));
        cv.stroke_ellipse(e, rgb(LINE), 1.0);
        if i == st.color {
            let g = 4.0 * s;
            cv.stroke_ellipse((e.0 - g, e.1 - g, e.2 + 2.0 * g, e.3 + 2.0 * g), rgb(ACCENT), 2.0 * s);
        }
    }
    // Barras deslizantes.
    for (i, (cy, lo, hi)) in SLIDERS.iter().enumerate() {
        let len = TRACK_X1 - TRACK_X0;
        cv.fill_round(sc((TRACK_X0, cy - 2.5, len, 5.0)), 2.5 * s, rgb(LINE));
        let f = slider_frac(st.vals[i], *lo, *hi);
        if f > 0.0 {
            cv.fill_round(sc((TRACK_X0, cy - 2.5, len * f, 5.0)), 2.5 * s, rgb(ACCENT));
        }
        let kx = TRACK_X0 + len * f;
        cv.fill_ellipse(sc((kx - 8.0, cy - 8.0, 16.0, 16.0)), rgb(ACCENT));
        cv.fill_ellipse(sc((kx - 3.0, cy - 3.0, 6.0, 6.0)), rgb(CARD));
    }
    // Interruptor.
    let tx = TOGGLE_ROW.0 + TOGGLE_ROW.2 - 38.0;
    let ty = TOGGLE_ROW.1 + 2.0;
    cv.fill_round(sc((tx, ty, 38.0, 22.0)), 11.0 * s, rgb(if st.startup { ACCENT } else { LINE }));
    let kx = if st.startup { tx + 19.0 } else { tx + 3.0 };
    cv.fill_ellipse(sc((kx, ty + 3.0, 16.0, 16.0)), rgb(0xFFFFFF));
    // Botões.
    cv.fill_round(sc(BTN_CANCEL), 8.0 * s, rgb(FIELD));
    cv.stroke_round(sc(BTN_CANCEL), 8.0 * s, rgb(LINE), 1.0);
    cv.fill_round(sc(BTN_SAVE), 8.0 * s, rgb(ACCENT));
}

fn texts(dc: HDC, st: &State) {
    let s = st.scale;
    let reg = font(s, 13.0, 400);
    let bold = font(s, 13.0, 600);
    let seg_reg = font(s, 12.0, 400);
    let seg_bold = font(s, 12.0, 600);
    let pick = |f: HFONT| unsafe {
        SelectObject(dc, HGDIOBJ(f.0));
    };
    unsafe {
        SetBkMode(dc, TRANSPARENT);
    }
    let old = unsafe { SelectObject(dc, HGDIOBJ(bold.0)) };
    txt(dc, s, "Atalho de captura", (28.0, 24.0, 300.0, 22.0), INK, DT_LEFT);
    txt(dc, s, "Desenho", (28.0, 112.0, 300.0, 22.0), INK, DT_LEFT);
    txt(dc, s, "Arquivo", (28.0, 338.0, 300.0, 22.0), INK, DT_LEFT);
    for (i, (_, name)) in HOTKEYS.iter().enumerate() {
        let on = i == st.hotkey;
        pick(if on { seg_bold } else { seg_reg });
        let (bx, bw) = seg_bounds(i);
        txt(dc, s, name, (bx, SEG.1, bw, SEG.3), if on { ACCENT_FG } else { INK }, DT_CENTER);
    }
    pick(reg);
    txt(dc, s, "Cor padrão", (28.0, 134.0, 200.0, 20.0), MUTED, DT_LEFT);
    let labels = [("Espessura do traço", " px"), ("Tamanho do texto", " px"), ("Qualidade JPEG", "")];
    for (i, (name, unit)) in labels.iter().enumerate() {
        let cy = SLIDERS[i].0;
        pick(reg);
        txt(dc, s, name, (28.0, cy - 34.0, 220.0, 20.0), MUTED, DT_LEFT);
        pick(bold);
        txt(dc, s, &format!("{}{}", st.vals[i], unit), (324.0, cy - 12.0, 48.0, 24.0), INK, DT_RIGHT);
    }
    pick(reg);
    txt(dc, s, "Iniciar com o Windows", (TOGGLE_ROW.0, TOGGLE_ROW.1, 250.0, TOGGLE_ROW.3), INK, DT_LEFT);
    pick(bold);
    txt(dc, s, "Cancelar", BTN_CANCEL, INK, DT_CENTER);
    txt(dc, s, "Salvar", BTN_SAVE, ACCENT_FG, DT_CENTER);
    unsafe {
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(reg.0));
        let _ = DeleteObject(HGDIOBJ(bold.0));
        let _ = DeleteObject(HGDIOBJ(seg_reg.0));
        let _ = DeleteObject(HGDIOBJ(seg_bold.0));
    }
}

fn paint(hwnd: HWND) {
    let Some(st) = with_state(|st| *st) else { return };
    let (pw, ph) = ((W as f32 * st.scale) as i32, (H as f32 * st.scale) as i32);
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let dc = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, pw, ph);
        let old = SelectObject(dc, HGDIOBJ(bmp.0));
        shapes(dc, &st, pw as f32, ph as f32); // o Canvas some aqui, antes do texto
        texts(dc, &st);
        let _ = BitBlt(hdc, 0, 0, pw, ph, Some(dc), 0, 0, SRCCOPY);
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(dc);
        let _ = EndPaint(hwnd, &ps);
    }
}

fn save(hwnd: HWND) {
    let Some(st) = with_state(|st| *st) else { return };
    let path = crate::settings_path();
    let mut s = Settings::load(&path);
    s.hotkey = HOTKEYS[st.hotkey.min(2)].0;
    s.default_color = COLORS[st.color.min(5)];
    s.stroke_width = st.vals[0];
    s.text_size = st.vals[1];
    s.jpeg_quality = st.vals[2] as u8;
    s.start_with_windows = st.startup;

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
        let p = crate::settings_path();
        let mut cur = Settings::load(&p);
        // Sem arquivo ainda: o estado real de "iniciar com o Windows" vem do registro.
        if !p.exists() {
            cur.start_with_windows = startup::is_enabled();
        }
        let scale = (GetDpiForSystem() as f32 / 96.0).max(1.0);
        if let Ok(mut g) = STATE.lock() {
            *g = Some(State {
                hotkey: HOTKEYS.iter().position(|(c, _)| *c == cur.hotkey).unwrap_or(0),
                color: COLORS.iter().position(|c| *c == cur.default_color).unwrap_or(3),
                vals: [
                    cur.stroke_width.clamp(1, 20),
                    cur.text_size.clamp(8, 96),
                    (cur.jpeg_quality as u32).clamp(1, 100),
                ],
                startup: cur.start_with_windows,
                drag: None,
                scale,
            });
        }

        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let style = WS_CAPTION | WS_SYSMENU;
        let mut r = RECT { left: 0, top: 0, right: (W as f32 * scale) as i32, bottom: (H as f32 * scale) as i32 };
        let _ = AdjustWindowRectEx(&mut r, style, false, WINDOW_EX_STYLE(0));
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let x = (GetSystemMetrics(SM_CXSCREEN) - ww) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - wh) / 2;
        let title = wide("Screenshotter — Configurações");
        let Ok(hwnd) = CreateWindowExW(
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
        ) else {
            return;
        };
        if let Ok(icon) = crate::tray::load_tray_icon() {
            // WM_SETICON (0x80), ICON_SMALL = 0: ícone do app na barra de título.
            let _ = SendMessageW(hwnd, 0x80, Some(WPARAM(0)), Some(LPARAM(icon.0 as isize)));
        }
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const i32 as *const _, 4);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_ends_and_clamp() {
        assert_eq!(slider_value(TRACK_X0 - 50.0, 1, 20), 1);
        assert_eq!(slider_value(TRACK_X1 + 50.0, 1, 20), 20);
        assert_eq!(slider_value((TRACK_X0 + TRACK_X1) / 2.0, 0, 100), 50);
    }

    #[test]
    fn slider_frac_bounds() {
        assert_eq!(slider_frac(1, 1, 20), 0.0);
        assert_eq!(slider_frac(20, 1, 20), 1.0);
        assert_eq!(slider_frac(999, 1, 100), 1.0);
    }

    #[test]
    fn hit_targets() {
        assert_eq!(hit(SEG.0 + 5.0, SEG.1 + 5.0), Some(Hit::Seg(0)));
        assert_eq!(hit(SEG.0 + SEG.2 - 5.0, SEG.1 + 5.0), Some(Hit::Seg(2)));
        assert_eq!(hit(SWATCH_X0 + SWATCH_STEP * 3.0, SWATCH_Y), Some(Hit::Swatch(3)));
        assert_eq!(hit(100.0, SLIDERS[1].0), Some(Hit::Slider(1)));
        assert_eq!(hit(60.0, TOGGLE_ROW.1 + 10.0), Some(Hit::Toggle));
        assert_eq!(hit(BTN_SAVE.0 + 5.0, BTN_SAVE.1 + 5.0), Some(Hit::Save));
        assert_eq!(hit(BTN_CANCEL.0 + 5.0, BTN_CANCEL.1 + 5.0), Some(Hit::Cancel));
        assert_eq!(hit(5.0, 5.0), None);
    }

    #[test]
    fn controls_stay_inside_their_cards() {
        let cards = [CARD_HOTKEY, CARD_DRAW, CARD_FILE];
        let in_card = |x: f32, y: f32| cards.iter().any(|c| inside(*c, x, y));
        assert!(in_card(SEG.0, SEG.1) && in_card(SEG.0 + SEG.2, SEG.1 + SEG.3 - 1.0));
        assert!(in_card(SWATCH_X0 + SWATCH_STEP * 5.0 + SWATCH_R + 4.0, SWATCH_Y));
        for (cy, _, _) in SLIDERS {
            assert!(in_card(TRACK_X0, cy) && in_card(TRACK_X1 + 50.0, cy));
        }
        assert!(in_card(TOGGLE_ROW.0 + TOGGLE_ROW.2 - 1.0, TOGGLE_ROW.1 + TOGGLE_ROW.3));
    }

    #[test]
    fn colorref_swaps_channels() {
        assert_eq!(colorref(0x112233).0, 0x0033_2211);
    }
}
