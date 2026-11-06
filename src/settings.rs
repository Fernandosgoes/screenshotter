//! Janela de configurações (aberta pelo menu da bandeja). Desenhada à mão, sempre em branco, no
//! layout aprovado: atalho (padrões e personalizado), cor, traço, texto, fonte, formato e
//! "iniciar com o Windows". Salvar grava settings.json, registra o atalho na hora e ajusta a
//! inicialização automática.

use crate::config::{Accent, Color, Hotkey, HotkeyChoice, ImageFormat, Settings, TextFont};
use crate::gfx::{Canvas, RectF, argb, argb_from_colorref};
use crate::{hotkey, startup};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, DRAW_TEXT_FORMAT, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_RIGHT,
    DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect, GetStockObject, HBRUSH, HDC, HGDIOBJ,
    InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_DASH, RoundRect, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, IDC_ARROW,
    LoadCursorW, MB_ICONINFORMATION, MB_OK, MessageBoxW, RegisterClassW, SW_SHOW, SWP_NOMOVE,
    SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_ACTION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW,
    SetForegroundWindow, SetWindowPos, ShowWindow, SystemParametersInfoW, WINDOW_EX_STYLE,
    WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WM_SYSKEYDOWN, WNDCLASSW, WS_CAPTION, WS_SYSMENU,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("ScreenshotterSettings");

// Medidas em pixels lógicos (96 DPI); a escala de DPI é aplicada na hora de desenhar.
const W: f32 = 420.0;
const M: f32 = 14.0;
const INNER: f32 = W - 2.0 * M;
const HEADER: f32 = 44.0;
const BTN_H: f32 = 30.0;
const GAP: f32 = 6.0;
/// Trilha das barras: começa depois do nome e termina antes do valor.
const TRACK_X: f32 = M + 64.0;
const TRACK_W: f32 = INNER - 64.0 - 68.0;

const STROKE_RANGE: (u32, u32) = (1, 12);
const TEXT_RANGE: (u32, u32) = (10, 48);

// Cores em 0xRRGGBB.
const INK: u32 = 0x171A24;
const MUTED: u32 = 0x636A7D;
const LINE: u32 = 0xD9DCE4;
const SURFACE2: u32 = 0xECEEF2;
const BTN_INK: u32 = 0x111111;
const WHITE: u32 = 0xFFFFFF;

type R = (f32, f32, f32, f32);
type P = (f32, f32);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Slider {
    Stroke,
    Text,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Hk(usize),
    Add,
    Dot(Accent),
    Swatch(usize),
    Slider(Slider),
    Font(usize),
    Format(usize),
    Startup,
    Cancel,
    Save,
}

#[derive(Clone)]
struct State {
    hotkey: HotkeyChoice,
    custom: Option<Hotkey>,
    /// Esperando o usuário apertar a combinação do atalho personalizado.
    capturing: bool,
    color: Color,
    stroke: u32,
    text: u32,
    font: TextFont,
    format: ImageFormat,
    startup: bool,
    accent: Accent,
    drag: Option<Slider>,
}

/// Estado da janela. Nunca segurar o lock ao enviar mensagens ou abrir caixas de diálogo
/// (o MessageBox roda um laço próprio e reentraria aqui).
static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Janela de configurações aberta (0 = nenhuma).
static OPEN: AtomicIsize = AtomicIsize::new(0);
/// Janela principal (dona do atalho global): precisa ser reregistrada ao salvar.
static MAIN: AtomicIsize = AtomicIsize::new(0);
/// Escala de DPI (bits de f32).
static SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

/// Posições de tudo, em pixels lógicos, para um número de atalhos (3 ou 4).
struct Lay {
    dots: [(Accent, R); 2],
    hk: Vec<R>,
    add: R,
    swatches: Vec<(Color, R)>,
    stroke_y: f32,
    stroke_track: R,
    stroke_value: R,
    text_y: f32,
    text_track: R,
    text_value: R,
    fonts: Vec<(TextFont, R)>,
    cap_y: f32,
    des_y: f32,
    sai_y: f32,
    fmt_y: f32,
    formats: Vec<(ImageFormat, R)>,
    startup_y: f32,
    startup: R,
    footer_top: f32,
    cancel: R,
    save: R,
    height: f32,
}

fn layout(n_hk: usize) -> Lay {
    let mut y = HEADER + 14.0;
    let cap_y = y;
    y += 16.0 + 8.0;
    let rows = n_hk.div_ceil(3);
    let col_w = (INNER - 2.0 * GAP) / 3.0;
    let hk = (0..n_hk)
        .map(|i| (M + (i % 3) as f32 * (col_w + GAP), y + (i / 3) as f32 * (BTN_H + GAP), col_w, BTN_H))
        .collect();
    y += rows as f32 * BTN_H + (rows as f32 - 1.0) * GAP + 10.0;
    let add = (M, y, INNER, 28.0);
    y += 28.0 + 18.0;
    let des_y = y;
    y += 16.0 + 8.0;
    let swatches = Color::ALL
        .iter()
        .enumerate()
        .map(|(i, c)| (*c, (M + i as f32 * 34.0, y, 24.0, 24.0)))
        .collect();
    y += 24.0 + 16.0;
    let stroke_y = y;
    let stroke_track = (TRACK_X, y, TRACK_W, 24.0);
    let stroke_value = (W - M - 60.0, y, 60.0, 24.0);
    y += 24.0 + 10.0;
    let text_y = y;
    let text_track = (TRACK_X, y, TRACK_W, 24.0);
    let text_value = (W - M - 60.0, y, 60.0, 24.0);
    y += 24.0 + 18.0;
    let fw = (INNER - 2.0 * GAP) / 3.0;
    let fonts = TextFont::ALL
        .iter()
        .enumerate()
        .map(|(i, f)| (*f, (M + i as f32 * (fw + GAP), y, fw, BTN_H)))
        .collect();
    y += BTN_H + 18.0;
    let sai_y = y;
    y += 16.0 + 8.0;
    let fmt_y = y;
    let fx = W - M - 152.0;
    let formats = vec![
        (ImageFormat::Jpg, (fx, y, 74.0, BTN_H)),
        (ImageFormat::Png, (fx + 78.0, y, 74.0, BTN_H)),
    ];
    y += BTN_H + 12.0;
    let startup_y = y;
    let startup = (W - M - 38.0, y + 4.0, 38.0, 22.0);
    y += BTN_H + 14.0;
    let footer_top = y;
    let cancel = (W - M - 96.0 - 8.0 - 96.0, footer_top + 14.0, 96.0, BTN_H);
    let save = (W - M - 96.0, footer_top + 14.0, 96.0, BTN_H);
    Lay {
        dots: [
            (Accent::Grafite, (W - M - 16.0 - 23.0, 14.0, 16.0, 16.0)),
            (Accent::Azul, (W - M - 16.0, 14.0, 16.0, 16.0)),
        ],
        hk,
        add,
        swatches,
        stroke_y,
        stroke_track,
        stroke_value,
        text_y,
        text_track,
        text_value,
        fonts,
        cap_y,
        des_y,
        sai_y,
        fmt_y,
        formats,
        startup_y,
        startup,
        footer_top,
        cancel,
        save,
        height: footer_top + 58.0,
    }
}

/// Atalhos mostrados: os três padrões e, se existir, o personalizado.
fn items_of(custom: Option<Hotkey>) -> Vec<(HotkeyChoice, String)> {
    let mut v = vec![
        (HotkeyChoice::ShiftBackspace, "Shift + Backspace".to_string()),
        (HotkeyChoice::ShiftInsert, "Shift + Insert".to_string()),
        (HotkeyChoice::ShiftPrtScn, "Shift + PrtScn".to_string()),
    ];
    if let Some(h) = custom {
        v.push((HotkeyChoice::Custom, hotkey_label(h)));
    }
    v
}

fn hotkey_label(h: Hotkey) -> String {
    let mut parts = Vec::new();
    if h.mods & 2 != 0 {
        parts.push("Ctrl".to_string());
    }
    if h.mods & 1 != 0 {
        parts.push("Alt".to_string());
    }
    if h.mods & 4 != 0 {
        parts.push("Shift".to_string());
    }
    parts.push(vk_name(h.vk));
    parts.join(" + ")
}

fn vk_name(vk: u32) -> String {
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x20 => "Espaço".into(),
        0x21 => "PgUp".into(),
        0x22 => "PgDn".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Esquerda".into(),
        0x26 => "Cima".into(),
        0x27 => "Direita".into(),
        0x28 => "Baixo".into(),
        0x2C => "PrtScn".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).unwrap_or('?').to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => format!("Tecla {vk:#04X}"),
    }
}

/// Aceita só combinações que o Windows aceita como atalho global e que não viram
/// digitação normal: precisa de Ctrl, Alt ou Shift (ou ser uma tecla F), e não pode ser
/// só um modificador.
fn valid_custom(h: Hotkey) -> Result<(), &'static str> {
    let modifier_only = matches!(h.vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5);
    if h.vk == 0 {
        return Err("Aperte a combinação de teclas.");
    }
    if modifier_only {
        return Err("Escolha uma tecla que não seja só Ctrl, Alt ou Shift.");
    }
    let f_key = (0x70..=0x87).contains(&h.vk);
    if h.mods == 0 && !f_key {
        return Err("Use Ctrl, Alt ou Shift junto com a tecla (ou uma tecla F).");
    }
    Ok(())
}

fn scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

fn snap() -> Option<State> {
    STATE.lock().ok()?.as_ref().cloned()
}

fn with<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut g = STATE.lock().ok()?;
    g.as_mut().map(f)
}

fn cr(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn send(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { SendMessageW(h, msg, Some(wp), Some(lp)) }
}

fn inside(r: R, p: P) -> bool {
    p.0 >= r.0 && p.0 <= r.0 + r.2 && p.1 >= r.1 && p.1 <= r.1 + r.3
}

fn grow(r: R, d: f32) -> R {
    (r.0 - d, r.1 - d, r.2 + 2.0 * d, r.3 + 2.0 * d)
}

fn sr(r: R, s: f32) -> RectF {
    (r.0 * s, r.1 * s, r.2 * s, r.3 * s)
}

fn prc(r: R, s: f32) -> RECT {
    RECT {
        left: (r.0 * s).round() as i32,
        top: (r.1 * s).round() as i32,
        right: ((r.0 + r.2) * s).round() as i32,
        bottom: ((r.1 + r.3) * s).round() as i32,
    }
}

fn hit_at(p: P, st: &State) -> Option<Hit> {
    let items = items_of(st.custom);
    let lay = layout(items.len());
    for (a, r) in lay.dots.iter() {
        if inside(*r, p) {
            return Some(Hit::Dot(*a));
        }
    }
    for (i, r) in lay.hk.iter().enumerate() {
        if inside(*r, p) {
            return Some(Hit::Hk(i));
        }
    }
    if inside(lay.add, p) {
        return Some(Hit::Add);
    }
    for (i, (_, r)) in lay.swatches.iter().enumerate() {
        if inside(*r, p) {
            return Some(Hit::Swatch(i));
        }
    }
    let track_hit = |t: R| (t.0 - 8.0, t.1 - 8.0, t.2 + 16.0, t.3 + 16.0);
    if inside(track_hit(lay.stroke_track), p) {
        return Some(Hit::Slider(Slider::Stroke));
    }
    if inside(track_hit(lay.text_track), p) {
        return Some(Hit::Slider(Slider::Text));
    }
    for (i, (_, r)) in lay.fonts.iter().enumerate() {
        if inside(*r, p) {
            return Some(Hit::Font(i));
        }
    }
    for (i, (_, r)) in lay.formats.iter().enumerate() {
        if inside(*r, p) {
            return Some(Hit::Format(i));
        }
    }
    if inside(grow(lay.startup, 4.0), p) {
        return Some(Hit::Startup);
    }
    if inside(lay.cancel, p) {
        return Some(Hit::Cancel);
    }
    if inside(lay.save, p) {
        return Some(Hit::Save);
    }
    None
}

/// Valor da barra conforme a posição x do mouse (já em pixels lógicos).
fn set_slider(sl: Slider, x: f32) {
    let f = ((x - TRACK_X) / TRACK_W).clamp(0.0, 1.0);
    let (lo, hi) = match sl {
        Slider::Stroke => STROKE_RANGE,
        Slider::Text => TEXT_RANGE,
    };
    let v = lo + (f * (hi - lo) as f32).round() as u32;
    with(|s| match sl {
        Slider::Stroke => s.stroke = v,
        Slider::Text => s.text = v,
    });
}

fn key_down(vk: i32) -> bool {
    unsafe { GetKeyState(vk) < 0 }
}

fn to_logical(lp: LPARAM) -> P {
    let s = scale();
    let x = (lp.0 & 0xFFFF) as i16 as f32;
    let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
    (x / s, y / s)
}

fn invalidate(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn window_size(client_h: f32, s: f32) -> (i32, i32) {
    let mut r = RECT { left: 0, top: 0, right: (W * s).round() as i32, bottom: (client_h * s).round() as i32 };
    unsafe {
        let _ = AdjustWindowRectEx(&mut r, WS_CAPTION | WS_SYSMENU, false, WINDOW_EX_STYLE(0));
    }
    (r.right - r.left, r.bottom - r.top)
}

/// Ajusta a altura da janela ao número de atalhos e redesenha.
fn refresh(hwnd: HWND) {
    let Some(st) = snap() else { return };
    let h = layout(items_of(st.custom).len()).height;
    let (ww, wh) = window_size(h, scale());
    unsafe {
        let _ = SetWindowPos(hwnd, None, 0, 0, ww, wh, SWP_NOMOVE | SWP_NOZORDER);
    }
    invalidate(hwnd);
}

fn on_down(hwnd: HWND, p: P) {
    let Some(st) = snap() else { return };
    let Some(hit) = hit_at(p, &st) else { return };
    let items = items_of(st.custom);
    match hit {
        Hit::Hk(i) => {
            let c = items[i].0;
            with(|s| {
                s.hotkey = c;
                s.capturing = false;
            });
        }
        Hit::Add => {
            with(|s| s.capturing = !s.capturing);
        }
        Hit::Dot(a) => {
            with(|s| s.accent = a);
        }
        Hit::Swatch(i) => {
            let c = Color::ALL[i];
            with(|s| s.color = c);
        }
        Hit::Slider(sl) => {
            with(|s| s.drag = Some(sl));
            unsafe {
                SetCapture(hwnd);
            }
            set_slider(sl, p.0);
        }
        Hit::Font(i) => {
            let f = TextFont::ALL[i];
            with(|s| s.font = f);
        }
        Hit::Format(i) => {
            let f = if i == 0 { ImageFormat::Jpg } else { ImageFormat::Png };
            with(|s| s.format = f);
        }
        Hit::Startup => {
            with(|s| s.startup = !s.startup);
        }
        Hit::Cancel => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return;
        }
        Hit::Save => {
            save(hwnd);
            return;
        }
    }
    refresh(hwnd);
}

fn on_move(hwnd: HWND, p: P) {
    if let Some(Some(sl)) = snap().map(|s| s.drag) {
        set_slider(sl, p.0);
        invalidate(hwnd);
    }
}

fn on_up() {
    with(|s| s.drag = None);
    unsafe {
        let _ = ReleaseCapture();
    }
}

fn on_key(hwnd: HWND, vk: u32) {
    let Some(st) = snap() else { return };
    if st.capturing {
        match vk {
            // Só modificador apertado sozinho: espera a tecla de verdade.
            0x10 | 0x11 | 0x12 | 0x5B | 0x5C => return,
            0x1B => {
                with(|s| s.capturing = false);
            }
            _ => {
                let mut mods = 0u32;
                if key_down(0x12) {
                    mods |= 1;
                }
                if key_down(0x11) {
                    mods |= 2;
                }
                if key_down(0x10) {
                    mods |= 4;
                }
                let h = Hotkey { mods, vk };
                match valid_custom(h) {
                    Ok(()) => {
                        with(|s| {
                            s.custom = Some(h);
                            s.hotkey = HotkeyChoice::Custom;
                            s.capturing = false;
                        });
                    }
                    Err(msg) => info(hwnd, msg),
                }
            }
        }
        refresh(hwnd);
    } else if vk == 0x1B {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    } else if vk == 0x0D {
        save(hwnd);
    }
}

fn save(hwnd: HWND) {
    let Some(st) = snap() else { return };
    if st.hotkey == HotkeyChoice::Custom {
        if let Some(h) = st.custom {
            if let Err(msg) = valid_custom(h) {
                info(hwnd, msg);
                return;
            }
        }
    }
    let path = crate::settings_path();
    let mut s = Settings::load(&path);
    s.hotkey = st.hotkey;
    if st.custom.is_some() {
        s.custom_hotkey = st.custom;
    }
    s.default_color = st.color;
    s.stroke_width = st.stroke;
    s.text_size = st.text;
    s.save_format = st.format;
    s.start_with_windows = st.startup;
    s.text_font = st.font;
    s.accent = st.accent;

    if s.save(&path).is_err() {
        info(hwnd, "Não foi possível gravar as configurações.");
        return;
    }
    let want = s.start_with_windows;
    std::thread::spawn(move || {
        let _ = startup::set_enabled(want);
    });
    let main = HWND(MAIN.load(Ordering::Relaxed) as *mut _);
    let (_, warn) = hotkey::register(main, &s);
    if let Some(w) = warn {
        info(hwnd, w);
    }
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

fn info(hwnd: HWND, text: &str) {
    let t = wide(text);
    unsafe {
        MessageBoxW(Some(hwnd), PCWSTR(t.as_ptr()), w!("Screenshotter"), MB_OK | MB_ICONINFORMATION);
    }
}

/// Texto desenhado com GDI: `r` em pixels lógicos, alinhado à esquerda, centro ou direita.
unsafe fn text(hdc: HDC, s: f32, r: R, t: &str, px: f32, weight: i32, face: PCWSTR, rgb: u32, align: DRAW_TEXT_FORMAT) {
    unsafe {
        let f = CreateFontW(
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
            FONT_QUALITY(5),
            0,
            face,
        );
        let old = SelectObject(hdc, HGDIOBJ(f.0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, cr(rgb));
        let mut rc = prc(r, s);
        let mut wt: Vec<u16> = t.encode_utf16().collect();
        let _ = DrawTextW(hdc, &mut wt, &mut rc, align | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(hdc, old);
        let _ = DeleteObject(HGDIOBJ(f.0));
    }
}

/// Contorno tracejado de cantos arredondados (o atalho personalizado e o botão de adicionar).
unsafe fn dashed(hdc: HDC, r: R, s: f32, rgb: u32) {
    unsafe {
        let pen = CreatePen(PS_DASH, 1, cr(rgb));
        let old_p = SelectObject(hdc, HGDIOBJ(pen.0));
        let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let rc = prc(r, s);
        let _ = RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, (16.0 * s) as i32, (16.0 * s) as i32);
        SelectObject(hdc, old_p);
        SelectObject(hdc, old_b);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

fn rrect(cv: &Canvas, r: R, rgb: u32, s: f32) {
    cv.fill_round(sr(r, s), 8.0 * s, argb(0xFF, rgb));
}

fn slider_paint(cv: &Canvas, track: R, v: u32, (lo, hi): (u32, u32), accent: u32, s: f32) {
    let f = (v - lo) as f32 / (hi - lo) as f32;
    let (x, y, w, h) = track;
    let cy = y + h / 2.0;
    cv.fill_round(sr((x, cy - 2.0, w, 4.0), s), 2.0 * s, argb(0xFF, LINE));
    if f > 0.0 {
        cv.fill_round(sr((x, cy - 2.0, w * f, 4.0), s), 2.0 * s, argb(0xFF, accent));
    }
    let kx = x + w * f;
    cv.fill_ellipse(sr((kx - 8.0, cy - 8.0, 16.0, 16.0), s), argb(0xFF, accent));
}

fn paint(hwnd: HWND) {
    let Some(st) = snap() else { return };
    let s = scale();
    let items = items_of(st.custom);
    let lay = layout(items.len());
    let accent = st.accent.rgb();
    let ui = w!("Segoe UI");
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        FillRect(hdc, &ps.rcPaint, HBRUSH(GetStockObject(WHITE_BRUSH).0));

        if let Some(cv) = Canvas::from_hdc(hdc) {
            // Divisória do cabeçalho e do rodapé.
            cv.fill_round(sr((0.0, HEADER - 1.0, W, 1.0), s), 0.0, argb(0xFF, LINE));
            cv.fill_round(sr((0.0, lay.footer_top, W, 1.0), s), 0.0, argb(0xFF, LINE));

            // Bolinhas de cor do painel; a escolhida ganha anel.
            for (a, r) in lay.dots.iter() {
                cv.fill_ellipse(sr(*r, s), argb(0xFF, a.rgb()));
                if *a == st.accent {
                    cv.stroke_ellipse(sr(grow(*r, 3.0), s), argb(0xFF, INK), 2.0 * s);
                }
            }

            // Atalhos: selecionado em cor de destaque, o resto cinza.
            for (i, (choice, _)) in items.iter().enumerate() {
                let sel = st.hotkey == *choice;
                rrect(&cv, lay.hk[i], if sel { accent } else { SURFACE2 }, s);
            }

            // Cores: contorno preto (preto leva branco); a escolhida ganha anel.
            for (c, r) in lay.swatches.iter() {
                let e = sr(*r, s);
                cv.fill_ellipse(e, argb_from_colorref(c.colorref()));
                let ol = if *c == Color::Black { WHITE } else { 0x000000 };
                cv.stroke_ellipse(e, argb(0xFF, ol), 1.2 * s);
                if *c == st.color {
                    cv.stroke_ellipse(sr(grow(*r, 3.0), s), argb(0xFF, accent), 2.0 * s);
                }
            }

            slider_paint(&cv, lay.stroke_track, st.stroke, STROKE_RANGE, accent, s);
            slider_paint(&cv, lay.text_track, st.text, TEXT_RANGE, accent, s);

            for (f, r) in lay.fonts.iter() {
                rrect(&cv, *r, if st.font == *f { accent } else { SURFACE2 }, s);
            }
            for (fm, r) in lay.formats.iter() {
                rrect(&cv, *r, if st.format == *fm { accent } else { SURFACE2 }, s);
            }

            // Interruptor "iniciar com o Windows".
            let (sx, sy, sw_, _) = lay.startup;
            cv.fill_round(sr(lay.startup, s), 11.0 * s, argb(0xFF, if st.startup { accent } else { LINE }));
            let kx = if st.startup { sx + sw_ - 3.0 - 16.0 } else { sx + 3.0 };
            cv.fill_ellipse(sr((kx, sy + 3.0, 16.0, 16.0), s), argb(0xFF, WHITE));

            rrect(&cv, lay.cancel, SURFACE2, s);
            rrect(&cv, lay.save, accent, s);
        }

        // Contornos tracejados: atalho personalizado (quando não selecionado) e botão de adicionar.
        for (i, (choice, _)) in items.iter().enumerate() {
            if *choice == HotkeyChoice::Custom && st.hotkey != HotkeyChoice::Custom {
                dashed(hdc, lay.hk[i], s, accent);
            }
        }
        dashed(hdc, lay.add, s, if st.capturing { accent } else { MUTED });

        // Textos.
        text(hdc, s, (M, 0.0, 200.0, HEADER), "Configurações", 13.0, 600, ui, INK, DT_LEFT);
        text(hdc, s, (M, lay.cap_y, INNER, 16.0), "CAPTURA", 11.0, 600, ui, MUTED, DT_LEFT);
        text(hdc, s, (M, lay.des_y, INNER, 16.0), "DESENHO", 11.0, 600, ui, MUTED, DT_LEFT);
        text(hdc, s, (M, lay.sai_y, INNER, 16.0), "SAÍDA", 11.0, 600, ui, MUTED, DT_LEFT);

        for (i, (choice, label)) in items.iter().enumerate() {
            let sel = st.hotkey == *choice;
            text(hdc, s, lay.hk[i], label, 12.0, 500, ui, if sel { WHITE } else { BTN_INK }, DT_CENTER);
        }

        let (add_label, add_rgb) = if st.capturing {
            ("Aperte as teclas… (Esc cancela)", accent)
        } else if st.custom.is_some() {
            ("Trocar personalizado", MUTED)
        } else {
            ("+ Personalizado", MUTED)
        };
        text(hdc, s, lay.add, add_label, 12.0, 500, ui, add_rgb, DT_CENTER);

        text(hdc, s, (M, lay.stroke_y, 56.0, 24.0), "Traço", 13.0, 400, ui, INK, DT_LEFT);
        text(hdc, s, lay.stroke_value, &format!("{} px", st.stroke), 13.0, 500, ui, INK, DT_RIGHT);
        text(hdc, s, (M, lay.text_y, 56.0, 24.0), "Texto", 13.0, 400, ui, INK, DT_LEFT);
        text(hdc, s, lay.text_value, &format!("{} pt", st.text), 13.0, 500, ui, INK, DT_RIGHT);

        for (f, r) in lay.fonts.iter() {
            let face = match f {
                TextFont::Bricolage => w!("Bricolage Grotesque"),
                TextFont::Poppins => w!("Poppins"),
                TextFont::Orbitron => w!("Orbitron"),
            };
            text(hdc, s, *r, f.family(), 12.0, 500, face, if st.font == *f { WHITE } else { BTN_INK }, DT_CENTER);
        }

        text(hdc, s, (M, lay.fmt_y, 120.0, BTN_H), "Formato", 13.0, 400, ui, INK, DT_LEFT);
        for (fm, r) in lay.formats.iter() {
            let label = if *fm == ImageFormat::Jpg { "JPG" } else { "PNG" };
            text(hdc, s, *r, label, 12.0, 500, ui, if st.format == *fm { WHITE } else { BTN_INK }, DT_CENTER);
        }

        text(hdc, s, (M, lay.startup_y, 300.0, BTN_H), "Iniciar com o Windows", 13.0, 400, ui, INK, DT_LEFT);

        text(hdc, s, lay.cancel, "Cancelar", 12.0, 500, ui, BTN_INK, DT_CENTER);
        text(hdc, s, lay.save, "Salvar", 12.0, 600, ui, WHITE, DT_CENTER);

        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            on_down(hwnd, to_logical(lp));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            on_move(hwnd, to_logical(lp));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            on_up();
            LRESULT(0)
        }
        WM_KEYDOWN => {
            on_key(hwnd, wp.0 as u32);
            LRESULT(0)
        }
        // Alt+tecla só vira tecla do atalho enquanto se captura; fora disso segue o padrão.
        WM_SYSKEYDOWN if snap().is_some_and(|s| s.capturing) => {
            on_key(hwnd, wp.0 as u32);
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
            if let Ok(mut g) = STATE.lock() {
                *g = None;
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
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

    let s = (unsafe { GetDpiForSystem() } as f32 / 96.0).max(1.0);
    SCALE.store(s.to_bits(), Ordering::Relaxed);
    if let Ok(mut g) = STATE.lock() {
        *g = Some(State {
            hotkey: cur.hotkey,
            custom: cur.custom_hotkey,
            capturing: false,
            color: cur.default_color,
            stroke: cur.stroke_width.clamp(STROKE_RANGE.0, STROKE_RANGE.1),
            text: cur.text_size.clamp(TEXT_RANGE.0, TEXT_RANGE.1),
            font: cur.text_font,
            format: cur.save_format,
            startup: cur.start_with_windows,
            accent: cur.accent,
            drag: None,
        });
    }

    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let h = layout(items_of(cur.custom_hotkey).len()).height;
        let (ww, wh) = window_size(h, s);
        // Centralizada na área de trabalho (acima da barra de tarefas).
        let mut wa = RECT::default();
        let _ = SystemParametersInfoW(
            SYSTEM_PARAMETERS_INFO_ACTION(48), // SPI_GETWORKAREA
            0,
            Some(&mut wa as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let x = wa.left + (wa.right - wa.left - ww) / 2;
        let y = wa.top + (wa.bottom - wa.top - wh) / 2;

        let title = wide("Screenshotter");
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU,
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

        if let Ok(icon) = crate::tray::load_tray_icon() {
            send(hwnd, 0x80, WPARAM(0), LPARAM(icon.0 as isize)); // WM_SETICON, ICON_SMALL
        }
        OPEN.store(hwnd.0 as isize, Ordering::Relaxed);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}
