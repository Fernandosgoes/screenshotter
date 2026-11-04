//! Janela de sobreposição (topmost, cobre a área virtual). Pré-criada e oculta.
//! Traduz mensagens Win32 em eventos do `Editor` e pinta o resultado com buffer duplo.
//!
//! Regra de re-entrância: nunca segurar o `RefCell` da sessão ao chamar Win32 que pode
//! enviar mensagens síncronas (SetWindowPos, diálogos modais).

use crate::capture::{self, Capture};
use crate::config::Settings;
use crate::draw;
use crate::editor::{Editor, Interaction, Tool};
use crate::export;
use crate::geom::{Point, Rect, dimension_label};
use crate::gfx::{Canvas, argb};
use crate::log;
use crate::model::Annotation;
use crate::toolbar::{self, Button};
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::time::Instant;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC,
    CreateDIBSection, CreatePen, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, GetStockObject, HBITMAP, HDC,
    HGDIOBJ, InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, Rectangle, RGBQUAD,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, HWND_TOPMOST, IDC_ARROW, IDC_CROSS, LoadCursorW,
    RegisterClassW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetCursor, SetForegroundWindow,
    SetWindowPos, ShowWindow, WM_CHAR, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_PAINT, WM_SETCURSOR, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("ScreenshotterOverlay");
/// Raio das alças da seleção, em pixels.
const HANDLE_R: f32 = 4.5;
/// MK_SHIFT do WM_MOUSEMOVE (wParam), valor fixo do Win32.
const MK_SHIFT_FLAG: usize = 0x0004;

pub struct Session {
    pub hwnd: HWND,
    pub editor: Editor,
    pub dim: Surface,
    pub bright: Surface,
    pub back: Surface,
    pub t0: Instant,
    pub first_frame_logged: bool,
    /// Botão da barra sob o mouse (realce).
    pub hover: Option<Button>,
}

/// DC + bitmap de memória com o conteúdo BGRA de uma captura (ou um buffer vazio).
pub struct Surface {
    pub dc: HDC,
    pub bmp: HBITMAP,
    old: HGDIOBJ,
}

impl Surface {
    unsafe fn new(w: i32, h: i32, pixels: Option<&[u8]>) -> Option<Surface> {
        unsafe {
            let dc = CreateCompatibleDC(None);
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                bmiColors: [RGBQUAD::default()],
            };
            let mut bits: *mut c_void = null_mut();
            let bmp = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            if let (Some(px), false) = (pixels, bits.is_null()) {
                std::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u8, px.len());
            }
            let old = SelectObject(dc, HGDIOBJ(bmp.0));
            Some(Surface { dc, bmp, old })
        }
    }

    unsafe fn free(self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bmp.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

thread_local! {
    static OVERLAY: RefCell<Option<HWND>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

/// Registra a classe e cria a janela oculta. Chamar uma vez no início.
pub fn create() -> windows::core::Result<HWND> {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None)?.into();
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_CROSS)?,
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let (origin, w, h) = capture::virtual_screen_rect();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            CLASS,
            w!("Screenshotter"),
            WS_POPUP,
            origin.x,
            origin.y,
            w,
            h,
            None,
            None,
            Some(hinst),
            None,
        )?;
        OVERLAY.with(|o| *o.borrow_mut() = Some(hwnd));
        Ok(hwnd)
    }
}

/// Atalho acionado: captura a tela e mostra a sobreposição já com a imagem congelada.
pub fn start_capture(settings: &Settings) -> windows::core::Result<()> {
    let t0 = Instant::now();
    let Some(hwnd) = OVERLAY.with(|o| *o.borrow()) else {
        return Ok(());
    };
    // Se já há uma captura aberta, descarta antes de começar outra.
    finish();

    let cap: Capture = capture::capture_virtual_screen()?;
    let (w, h) = (cap.width, cap.height);
    let dim_px = capture::dimmed(&cap.pixels);
    let (dim, bright, back) = unsafe {
        (
            Surface::new(w, h, Some(&dim_px)),
            Surface::new(w, h, Some(&cap.pixels)),
            Surface::new(w, h, None),
        )
    };
    let (Some(dim), Some(bright), Some(back)) = (dim, bright, back) else {
        return Ok(());
    };
    let origin = cap.origin;
    let session = Session {
        hwnd,
        editor: Editor::new(cap, settings),
        dim,
        bright,
        back,
        t0,
        first_frame_logged: false,
        hover: None,
    };
    SESSION.with(|s| *s.borrow_mut() = Some(session));

    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), origin.x, origin.y, w, h, SWP_SHOWWINDOW | SWP_NOACTIVATE);
        let _ = SetForegroundWindow(hwnd);
        SetCapture(hwnd);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
    Ok(())
}

/// Esconde a sobreposição e libera a captura. Não grava nada em disco.
pub fn finish() {
    let taken = SESSION.with(|s| s.borrow_mut().take());
    if let Some(sess) = taken {
        unsafe {
            let _ = ReleaseCapture();
            let _ = ShowWindow(sess.hwnd, SW_HIDE);
            sess.dim.free();
            sess.bright.free();
            sess.back.free();
        }
    }
}

fn lparam_point(lp: LPARAM) -> Point {
    Point {
        x: (lp.0 & 0xFFFF) as i16 as i32,
        y: ((lp.0 >> 16) & 0xFFFF) as i16 as i32,
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            paint_to(hdc);
            unsafe { let _ = EndPaint(hwnd, &ps); }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            on_mouse_down(lparam_point(lp));
            redraw(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let shift = (wp.0 & MK_SHIFT_FLAG) != 0 || key_down(VK_SHIFT.0 as i32);
            let p = lparam_point(lp);
            SESSION.with(|s| {
                if let Some(sess) = s.borrow_mut().as_mut() {
                    sess.editor.on_mouse_move(p, shift);
                    sess.hover = sess.editor.sel.and_then(|sel| {
                        toolbar::hit(&toolbar::layout(sel, sess.editor.screen_rect()), p)
                    });
                }
            });
            redraw(hwnd);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            // Cruz só enquanto a seleção está sendo criada; depois, seta normal.
            // Sempre definimos o cursor aqui: evita o "carregando" do Windows ao iniciar o app.
            let crosshair = SESSION.with(|s| match s.try_borrow() {
                Ok(g) => g.as_ref().is_none_or(|sess| {
                    sess.editor.sel.is_none()
                        || matches!(sess.editor.state, Interaction::Selecting { .. })
                }),
                Err(_) => true,
            });
            unsafe {
                let id = if crosshair { IDC_CROSS } else { IDC_ARROW };
                if let Ok(cursor) = LoadCursorW(None, id) {
                    SetCursor(Some(cursor));
                }
            }
            LRESULT(1)
        }
        WM_LBUTTONUP => {
            SESSION.with(|s| {
                if let Some(sess) = s.borrow_mut().as_mut() {
                    sess.editor.on_mouse_up();
                }
            });
            redraw(hwnd);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp.0 >> 16) & 0xFFFF) as i16;
            SESSION.with(|s| {
                if let Some(sess) = s.borrow_mut().as_mut() {
                    sess.editor.on_wheel(delta);
                }
            });
            redraw(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            on_key_down(wp.0 as u32);
            redraw(hwnd);
            LRESULT(0)
        }
        WM_CHAR => {
            on_char(wp.0 as u32);
            redraw(hwnd);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn key_down(vk: i32) -> bool {
    unsafe { GetKeyState(vk) < 0 }
}

fn redraw(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

#[derive(Clone, Copy)]
enum Cmd {
    Copy,
    Save,
    Close,
}

fn on_mouse_down(p: Point) {
    // Decisão primeiro, sem segurar o borrow durante comandos que chamam Win32 modal.
    let cmd = SESSION.with(|s| {
        let mut g = s.borrow_mut();
        let sess = g.as_mut()?;
        let e = &mut sess.editor;
        if let Some(sel) = e.sel {
            let l = toolbar::layout(sel, e.screen_rect());
            if let Some(b) = toolbar::hit(&l, p) {
                return match b {
                    Button::Pen => { e.set_tool(Tool::Pen); None }
                    Button::Line => { e.set_tool(Tool::Line); None }
                    Button::Arrow => { e.set_tool(Tool::Arrow); None }
                    Button::Rect => { e.set_tool(Tool::Rect); None }
                    Button::Ellipse => { e.set_tool(Tool::Ellipse); None }
                    Button::Marker => { e.set_tool(Tool::Marker); None }
                    Button::Text => { e.set_tool(Tool::Text); None }
                    Button::Pixelate => { e.set_tool(Tool::Pixelate); None }
                    Button::Number => { e.set_tool(Tool::Number); None }
                    Button::ColorSwatch => { e.toggle_palette(); None }
                    Button::Undo => { e.undo(); None }
                    Button::Copy => Some(Cmd::Copy),
                    Button::Save => Some(Cmd::Save),
                    Button::Close => Some(Cmd::Close),
                };
            }
            if e.palette_open {
                if let Some(c) = toolbar::palette_hit(&l, p) {
                    e.set_color(c);
                    return None;
                }
            }
        }
        e.on_mouse_down(p);
        None
    });
    match cmd {
        Some(Cmd::Copy) => copy_and_close(),
        Some(Cmd::Save) => save_and_close(),
        Some(Cmd::Close) => finish(),
        None => {}
    }
}

fn on_key_down(vk: u32) {
    let ctrl = key_down(VK_CONTROL.0 as i32);
    let cmd = SESSION.with(|s| {
        let mut g = s.borrow_mut();
        let sess = g.as_mut()?;
        let e = &mut sess.editor;
        match vk {
            v if v == VK_ESCAPE.0 as u32 => {
                if e.text.is_some() {
                    e.commit_text();
                    None
                } else {
                    Some(Cmd::Close)
                }
            }
            v if v == VK_DELETE.0 as u32 => {
                e.delete_selected();
                None
            }
            0x5A if ctrl => {
                e.undo();
                None
            }
            0x59 if ctrl => {
                e.redo();
                None
            }
            0x43 if ctrl && e.has_selection() => Some(Cmd::Copy),
            0x53 if ctrl && e.has_selection() => Some(Cmd::Save),
            _ => None,
        }
    });
    match cmd {
        Some(Cmd::Copy) => copy_and_close(),
        Some(Cmd::Save) => save_and_close(),
        Some(Cmd::Close) => finish(),
        None => {}
    }
}

fn on_char(c: u32) {
    let Some(ch) = char::from_u32(c) else { return };
    SESSION.with(|s| {
        let mut g = s.borrow_mut();
        let Some(sess) = g.as_mut() else { return };
        let e = &mut sess.editor;
        if e.text.is_some() {
            e.on_char(ch);
        } else if ch.is_ascii_alphabetic() {
            e.on_key_letter(ch);
        } else if ch.is_ascii_digit() {
            e.on_key_digit(ch);
        }
    });
}

/// Pinta a cena inteira no buffer de fundo e copia para a janela.
fn paint_to(hdc: HDC) {
    SESSION.with(|s| {
        let mut g = s.borrow_mut();
        let Some(sess) = g.as_mut() else { return };
        let (w, h) = (sess.editor.capture.width, sess.editor.capture.height);
        let e = &sess.editor;
        unsafe {
            let b = sess.back.dc;
            let _ = BitBlt(b, 0, 0, w, h, Some(sess.dim.dc), 0, 0, SRCCOPY);
            if let Some(sel) = e.sel {
                let _ = BitBlt(b, sel.x, sel.y, sel.w, sel.h, Some(sess.bright.dc), sel.x, sel.y, SRCCOPY);
            }
            draw::paint_annotations(b, e.doc.items(), 0, 0, true);
            if let Some(t) = &e.text {
                let a = Annotation::Text {
                    at: t.at,
                    text: t.buf.clone(),
                    color: e.color,
                    size: e.text_size,
                    background: e.text_background,
                };
                draw::paint_annotation(b, &a, 0, 0, false);
            }
            // Canvas suavizado (GDI+) para alças, etiqueta e barras.
            let canvas = Canvas::from_hdc(b);
            if let Some(sel) = e.sel {
                paint_selection(b, canvas.as_ref(), sel, e.capture.width);
                if let Some(cv) = canvas.as_ref() {
                    let l = toolbar::layout(sel, e.screen_rect());
                    let (v, hz) = toolbar::button_rects(&l);
                    let active = active_button(e.tool);
                    toolbar::paint_panel(cv, l.vertical);
                    toolbar::paint_buttons(cv, &v, Some(active), sess.hover, e.color);
                    toolbar::paint_panel(cv, l.horizontal);
                    toolbar::paint_buttons(cv, &hz, None, sess.hover, e.color);
                    if e.palette_open {
                        toolbar::paint_palette(
                            cv,
                            toolbar::palette_panel(&l),
                            &toolbar::palette_rects(&l),
                            e.color,
                        );
                    }
                }
            }
            drop(canvas);
            let _ = BitBlt(hdc, 0, 0, w, h, Some(b), 0, 0, SRCCOPY);
        }
        if !sess.first_frame_logged {
            sess.first_frame_logged = true;
            log::latency("atalho->quadro", sess.t0);
        }
    });
}

fn active_button(t: Tool) -> Button {
    match t {
        Tool::Pen => Button::Pen,
        Tool::Line => Button::Line,
        Tool::Arrow => Button::Arrow,
        Tool::Rect => Button::Rect,
        Tool::Ellipse => Button::Ellipse,
        Tool::Marker => Button::Marker,
        Tool::Text => Button::Text,
        Tool::Pixelate => Button::Pixelate,
        Tool::Number => Button::Number,
    }
}

/// Moldura contínua, alças redondas e etiqueta de medidas em formato de pílula.
unsafe fn paint_selection(hdc: HDC, cv: Option<&Canvas>, sel: Rect, screen_w: i32) {
    unsafe {
        // Traçado contínuo estilo Lightshot: linha escura por fora, branca por dentro.
        let outer = CreatePen(PS_SOLID, 1, COLORREF(0x0000_0000));
        let inner = CreatePen(PS_SOLID, 1, COLORREF(0x00FF_FFFF));
        let old_pen = SelectObject(hdc, HGDIOBJ(outer.0));
        let old_brush = SelectObject(hdc, HGDIOBJ(GetStockObject(NULL_BRUSH).0));
        let _ = Rectangle(hdc, sel.x - 1, sel.y - 1, sel.right() + 1, sel.bottom() + 1);
        SelectObject(hdc, HGDIOBJ(inner.0));
        let _ = Rectangle(hdc, sel.x, sel.y, sel.right(), sel.bottom());
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(outer.0));
        let _ = DeleteObject(HGDIOBJ(inner.0));

        let Some(cv) = cv else { return };

        // Alças: bolinhas brancas com contorno escuro suave.
        for (_, p) in sel.handles() {
            let r = (p.x as f32 - HANDLE_R, p.y as f32 - HANDLE_R, HANDLE_R * 2.0, HANDLE_R * 2.0);
            cv.fill_ellipse(r, argb(0xFF, 0xFFFFFF));
            cv.stroke_ellipse(r, argb(0xB0, 0x0F172A), 1.4);
        }

        // Etiqueta de medidas: acima do canto superior esquerdo, ou dentro se não couber.
        let text = dimension_label(sel);
        let label_w = text.len() as i32 * 8 + 18;
        let label_h = 22;
        let mut lx = sel.x;
        let mut ly = sel.y - label_h - 8;
        if ly < 0 {
            ly = sel.y + 8;
        }
        if lx + label_w > screen_w {
            lx = (screen_w - label_w).max(0);
        }
        cv.fill_round(
            (lx as f32, ly as f32, label_w as f32, label_h as f32),
            label_h as f32 / 2.0,
            argb(0xE6, 0x0F172A),
        );
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(0x00FF_FFFF));
        let mut rc = RECT { left: lx, top: ly, right: lx + label_w, bottom: ly + label_h };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(hdc, &mut wide, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }
}

fn copy_and_close() {
    let data = SESSION.with(|s| {
        s.borrow().as_ref().and_then(|sess| {
            let sel = sess.editor.sel?;
            let bgra = export::compose(&sess.editor.capture, sel, &sess.editor.doc);
            Some((sess.hwnd, bgra, sel.w, sel.h, sess.t0))
        })
    });
    if let Some((hwnd, bgra, w, h, t0)) = data {
        let _ = export::copy_to_clipboard(hwnd, &bgra, w, h);
        log::latency("ctrl+c->clipboard", t0);
        finish();
    }
}

fn save_and_close() {
    let data = SESSION.with(|s| {
        s.borrow().as_ref().and_then(|sess| {
            let sel = sess.editor.sel?;
            let bgra = export::compose(&sess.editor.capture, sel, &sess.editor.doc);
            Some((sess.hwnd, bgra, sel.w, sel.h))
        })
    });
    let Some((hwnd, bgra, w, h)) = data else { return };
    // Diálogo modal: nenhum borrow ativo aqui.
    if let Some(path) = export::save_dialog(hwnd) {
        if export::to_png(&bgra, w as u32, h as u32, &path).is_ok() {
            finish();
        }
    }
}
