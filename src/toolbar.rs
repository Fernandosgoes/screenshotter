//! Barras de ícones (PRD F7) e paleta de 6 cores (F11).
//! Visual: painéis escuros arredondados, botões de 40 px, destaque no botão ativo
//! e realce no hover. Layout puro + pintura GDI.

use crate::config::Color;
use crate::draw;
use crate::geom::{BarLayout, GAP, Point, Rect, layout_bars, shift_inside};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    CreatePen, CreateSolidBrush, DeleteObject, Ellipse, GetStockObject, HDC, HGDIOBJ, LineTo,
    MoveToEx, NULL_BRUSH, PS_NULL, PS_SOLID, Polyline, Rectangle, RoundRect, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};

/// Tamanho de um botão, em pixels.
pub const BTN: i32 = 40;
/// Margem interna do painel em volta dos botões.
pub const PAD: i32 = 6;
const SWATCH: i32 = 24;
const SWATCH_GAP: i32 = 6;
const RADIUS: i32 = 16;

// Cores em COLORREF (0x00BBGGRR).
const PANEL: u32 = 0x003B_291E; // #1E293B
const PANEL_HOVER: u32 = 0x0055_4133; // #334155
const ACCENT: u32 = 0x00E5_464F; // #4F46E5
const INK: u32 = 0x00F9_F5F1; // #F1F5F9
const RING: u32 = 0x00B8_A394; // #94A3B8

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Pen,
    Line,
    Arrow,
    Rect,
    Ellipse,
    Marker,
    Text,
    Pixelate,
    Number,
    ColorSwatch,
    Undo,
    Copy,
    Save,
    Close,
}

pub const VERTICAL: [Button; 11] = [
    Button::Pen,
    Button::Line,
    Button::Arrow,
    Button::Rect,
    Button::Ellipse,
    Button::Marker,
    Button::Text,
    Button::Pixelate,
    Button::Number,
    Button::ColorSwatch,
    Button::Undo,
];

pub const HORIZONTAL: [Button; 3] = [Button::Copy, Button::Save, Button::Close];

fn pad(r: Rect) -> Rect {
    Rect { x: r.x - PAD, y: r.y - PAD, w: r.w + 2 * PAD, h: r.h + 2 * PAD }
}

/// Painéis (com margem) das duas barras, sempre inteiramente dentro de `screen`.
pub fn layout(sel: Rect, screen: Rect) -> BarLayout {
    let base = layout_bars(sel, screen, VERTICAL.len() as i32, HORIZONTAL.len() as i32, BTN);
    BarLayout {
        vertical: shift_inside(pad(base.vertical), screen),
        horizontal: shift_inside(pad(base.horizontal), screen),
    }
}

/// Retângulo de cada botão, na ordem de `VERTICAL` e `HORIZONTAL`.
pub fn button_rects(l: &BarLayout) -> (Vec<(Button, Rect)>, Vec<(Button, Rect)>) {
    let v = VERTICAL
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (*b, Rect { x: l.vertical.x + PAD, y: l.vertical.y + PAD + i as i32 * BTN, w: BTN, h: BTN })
        })
        .collect();
    let h = HORIZONTAL
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (*b, Rect { x: l.horizontal.x + PAD + i as i32 * BTN, y: l.horizontal.y + PAD, w: BTN, h: BTN })
        })
        .collect();
    (v, h)
}

pub fn hit(l: &BarLayout, p: Point) -> Option<Button> {
    let (v, h) = button_rects(l);
    v.into_iter().chain(h).find(|(_, r)| r.contains(p)).map(|(b, _)| b)
}

/// Painel da paleta: abre à esquerda da barra vertical, na linha do quadradinho de cor.
pub fn palette_panel(l: &BarLayout) -> Rect {
    let row = VERTICAL.iter().position(|b| *b == Button::ColorSwatch).unwrap_or(9) as i32;
    let w = Color::ALL.len() as i32 * (SWATCH + SWATCH_GAP) - SWATCH_GAP + 2 * PAD;
    let h = SWATCH + 2 * PAD;
    let center_y = l.vertical.y + PAD + row * BTN + BTN / 2;
    Rect { x: (l.vertical.x - GAP - w).max(0), y: center_y - h / 2, w, h }
}

pub fn palette_rects(l: &BarLayout) -> Vec<(Color, Rect)> {
    let p = palette_panel(l);
    Color::ALL
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (*c, Rect { x: p.x + PAD + i as i32 * (SWATCH + SWATCH_GAP), y: p.y + PAD, w: SWATCH, h: SWATCH })
        })
        .collect()
}

pub fn palette_hit(l: &BarLayout, p: Point) -> Option<Color> {
    palette_rects(l).into_iter().find(|(_, r)| r.contains(p)).map(|(c, _)| c)
}

/// Preenche um retângulo arredondado sem contorno.
unsafe fn fill_round(hdc: HDC, r: Rect, color: u32, radius: i32) {
    unsafe {
        let brush = CreateSolidBrush(COLORREF(color));
        let pen = CreatePen(PS_NULL, 0, COLORREF(0));
        let ob = SelectObject(hdc, HGDIOBJ(brush.0));
        let op = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(hdc, r.x, r.y, r.right(), r.bottom(), radius, radius);
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

/// Fundo do painel (uma das barras ou a paleta).
pub unsafe fn paint_panel(hdc: HDC, panel: Rect) {
    unsafe { fill_round(hdc, panel, PANEL, RADIUS) };
}

/// Pinta os botões de uma barra: destaque no ativo, realce no hover, ícones.
pub unsafe fn paint_buttons(
    hdc: HDC,
    buttons: &[(Button, Rect)],
    active: Option<Button>,
    hover: Option<Button>,
    current: Color,
) {
    unsafe {
        for (b, r) in buttons {
            if Some(*b) == active {
                fill_round(hdc, *r, ACCENT, 12);
            } else if Some(*b) == hover {
                fill_round(hdc, *r, PANEL_HOVER, 12);
            }
            draw_icon(hdc, *b, *r, current);
        }
    }
}

/// Paleta: bolinhas com a cor; a cor ativa tem anel de destaque.
pub unsafe fn paint_palette(hdc: HDC, panel: Rect, swatches: &[(Color, Rect)], current: Color) {
    unsafe {
        paint_panel(hdc, panel);
        for (c, r) in swatches {
            let brush = CreateSolidBrush(COLORREF(c.colorref()));
            let pen = CreatePen(PS_SOLID, 1, COLORREF(RING));
            let ob = SelectObject(hdc, HGDIOBJ(brush.0));
            let op = SelectObject(hdc, HGDIOBJ(pen.0));
            let _ = Ellipse(hdc, r.x, r.y, r.right(), r.bottom());
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            let _ = DeleteObject(HGDIOBJ(brush.0));
            let _ = DeleteObject(HGDIOBJ(pen.0));
            if *c == current {
                let ring = CreatePen(PS_SOLID, 2, COLORREF(INK));
                let stock_null = GetStockObject(NULL_BRUSH);
                let old_brush = SelectObject(hdc, stock_null);
                let old_pen = SelectObject(hdc, HGDIOBJ(ring.0));
                let _ = Ellipse(hdc, r.x - 4, r.y - 4, r.right() + 4, r.bottom() + 4);
                SelectObject(hdc, old_pen);
                SelectObject(hdc, old_brush);
                let _ = DeleteObject(HGDIOBJ(ring.0));
            }
        }
    }
}

unsafe fn line(hdc: HDC, a: (i32, i32), b: (i32, i32)) {
    unsafe {
        let _ = MoveToEx(hdc, a.0, a.1, None);
        let _ = LineTo(hdc, b.0, b.1);
    }
}

/// Ícones desenhados com primitivas GDI, traço de 2 px em cor clara.
unsafe fn draw_icon(hdc: HDC, b: Button, r: Rect, current: Color) {
    unsafe {
        let inset = 11;
        let cx = r.x + r.w / 2;
        let cy = r.y + r.h / 2;
        let (x0, x1, y0, y1) = (r.x + inset, r.x + r.w - inset, r.y + inset, r.y + r.h - inset);
        let ink = CreatePen(PS_SOLID, 2, COLORREF(INK));
        let hollow_pen_old = SelectObject(hdc, HGDIOBJ(ink.0));
        let null_brush = windows::Win32::Graphics::Gdi::GetStockObject(windows::Win32::Graphics::Gdi::NULL_BRUSH);
        let old_brush = SelectObject(hdc, null_brush);
        match b {
            Button::Pen => {
                let pts = [
                    POINT { x: x0, y: y1 },
                    POINT { x: x0 + 5, y: y0 + 4 },
                    POINT { x: cx, y: y1 - 4 },
                    POINT { x: x1, y: y0 },
                ];
                let _ = Polyline(hdc, &pts);
            }
            Button::Line => line(hdc, (x0, y1), (x1, y0)),
            Button::Arrow => {
                line(hdc, (x0, y1), (x1, y0));
                line(hdc, (x1, y0), (x1 - 7, y0 + 1));
                line(hdc, (x1, y0), (x1 - 1, y0 + 7));
            }
            Button::Rect => {
                let _ = Rectangle(hdc, x0, y0 + 2, x1, y1 - 2);
            }
            Button::Ellipse => {
                let _ = Ellipse(hdc, x0, y0 + 2, x1, y1 - 2);
            }
            Button::Marker => {
                draw::fill_alpha(hdc, Rect { x: x0, y: cy - 5, w: x1 - x0, h: 10 }, 0x0000_FFFF, 170);
            }
            Button::Text => {
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(INK));
                let wide: Vec<u16> = "T".encode_utf16().collect();
                let _ = TextOutW(hdc, cx - 5, cy - 9, &wide);
            }
            Button::Pixelate => {
                for gy in 0..3 {
                    for gx in 0..3 {
                        if (gx + gy) % 2 == 0 {
                            let sx = x0 + gx * 6;
                            let sy = y0 + gy * 6;
                            let _ = Rectangle(hdc, sx, sy, sx + 5, sy + 5);
                        }
                    }
                }
            }
            Button::Number => {
                let _ = Ellipse(hdc, cx - 9, cy - 9, cx + 9, cy + 9);
            }
            Button::ColorSwatch => {
                let brush = CreateSolidBrush(COLORREF(current.colorref()));
                let old = SelectObject(hdc, HGDIOBJ(brush.0));
                let _ = Ellipse(hdc, x0 + 1, y0 + 1, x1 - 1, y1 - 1);
                SelectObject(hdc, old);
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            Button::Undo => {
                line(hdc, (x1, cy), (x0 + 4, cy));
                line(hdc, (x0 + 4, cy), (x0 + 9, cy - 5));
                line(hdc, (x0 + 4, cy), (x0 + 9, cy + 5));
            }
            Button::Copy => {
                let _ = Rectangle(hdc, x0, y0, x0 + 11, y0 + 11);
                let _ = Rectangle(hdc, x0 + 6, y0 + 6, x1, y1);
            }
            Button::Save => {
                let _ = Rectangle(hdc, x0, y0, x1, y1);
                let _ = Rectangle(hdc, x0 + 4, y0, x1 - 4, y0 + 6);
            }
            Button::Close => {
                line(hdc, (x0, y0), (x1, y1));
                line(hdc, (x1, y0), (x0, y1));
            }
        }
        SelectObject(hdc, old_brush);
        SelectObject(hdc, hollow_pen_old);
        let _ = DeleteObject(HGDIOBJ(ink.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panels_fit_inside_screen() {
        let screen = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        let sel = Rect { x: 1800, y: 900, w: 100, h: 100 };
        let l = layout(sel, screen);
        assert!(l.vertical.right() <= screen.right());
        assert!(l.vertical.bottom() <= screen.bottom());
        assert!(l.horizontal.right() <= screen.right());
        assert!(l.horizontal.bottom() <= screen.bottom());
    }

    #[test]
    fn tall_bar_near_top_keeps_full_height() {
        // Seleção perto do topo: a barra vertical (11 botões) não pode ser encolhida.
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 100, y: 20, w: 200, h: 50 };
        let l = layout(sel, screen);
        let needed = VERTICAL.len() as i32 * BTN + 2 * PAD;
        assert_eq!(l.vertical.h, needed);
        assert!(l.vertical.y >= 0 && l.vertical.bottom() <= screen.bottom());
    }

    #[test]
    fn buttons_stay_inside_their_panel() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 100, y: 100, w: 200, h: 200 };
        let l = layout(sel, screen);
        let (v, h) = button_rects(&l);
        for (_, r) in &v {
            assert!(l.vertical.contains(Point { x: r.x, y: r.y }));
        }
        for (_, r) in &h {
            assert!(l.horizontal.contains(Point { x: r.x, y: r.y }));
        }
    }

    #[test]
    fn hit_returns_correct_button() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 100, y: 100, w: 200, h: 200 };
        let l = layout(sel, screen);
        let (v, _) = button_rects(&l);
        let (b, r) = v[2];
        assert_eq!(hit(&l, Point { x: r.x + 1, y: r.y + 1 }), Some(b));
        assert_eq!(b, Button::Arrow);
    }

    #[test]
    fn palette_hit_maps_six_colors() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 300, y: 300, w: 200, h: 200 };
        let l = layout(sel, screen);
        let rects = palette_rects(&l);
        assert_eq!(rects.len(), 6);
        for (c, r) in rects {
            assert_eq!(palette_hit(&l, Point { x: r.x + 2, y: r.y + 2 }), Some(c));
        }
    }
}
