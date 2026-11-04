//! Barras de ícones (PRD F7) e paleta de 6 cores (F11). Layout puro + pintura GDI.

use crate::config::Color;
use crate::draw;
use crate::geom::{BarLayout, Point, Rect, layout_bars};
use windows::Win32::Foundation::{COLORREF, POINT};
use windows::Win32::Graphics::Gdi::{
    CreatePen, CreateSolidBrush, DeleteObject, Ellipse, GetStockObject, HDC, HGDIOBJ, LineTo,
    MoveToEx, NULL_BRUSH, Polyline, PS_SOLID, Rectangle, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT, TextOutW,
};

pub const ITEM: i32 = 32;
pub const SWATCH: i32 = 22;

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

pub fn layout(sel: Rect, screen: Rect) -> BarLayout {
    layout_bars(sel, screen, VERTICAL.len() as i32, HORIZONTAL.len() as i32, ITEM)
}

/// Retângulo de cada botão, na ordem de `VERTICAL` e `HORIZONTAL`.
pub fn button_rects(l: &BarLayout) -> (Vec<(Button, Rect)>, Vec<(Button, Rect)>) {
    let v = VERTICAL
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (*b, Rect { x: l.vertical.x, y: l.vertical.y + i as i32 * ITEM, w: ITEM, h: ITEM })
        })
        .collect();
    let h = HORIZONTAL
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (*b, Rect { x: l.horizontal.x + i as i32 * ITEM, y: l.horizontal.y, w: ITEM, h: ITEM })
        })
        .collect();
    (v, h)
}

pub fn hit(l: &BarLayout, p: Point) -> Option<Button> {
    let (v, h) = button_rects(l);
    v.into_iter().chain(h).find(|(_, r)| r.contains(p)).map(|(b, _)| b)
}

/// Paleta abre ao lado da barra vertical, na linha do quadradinho de cor.
pub fn palette_rects(l: &BarLayout) -> Vec<(Color, Rect)> {
    let swatch_row = VERTICAL.iter().position(|b| *b == Button::ColorSwatch).unwrap_or(9) as i32;
    let y = l.vertical.y + swatch_row * ITEM + (ITEM - SWATCH) / 2;
    let total = Color::ALL.len() as i32 * (SWATCH + 4);
    let x0 = (l.vertical.x - total - 4).max(0);
    Color::ALL
        .iter()
        .enumerate()
        .map(|(i, c)| (*c, Rect { x: x0 + i as i32 * (SWATCH + 4), y, w: SWATCH, h: SWATCH }))
        .collect()
}

pub fn palette_hit(l: &BarLayout, p: Point) -> Option<Color> {
    palette_rects(l).into_iter().find(|(_, r)| r.contains(p)).map(|(c, _)| c)
}

/// Pinta uma barra inteira (fundo branco, borda cinza, botão ativo em azul claro).
pub unsafe fn paint_buttons(
    hdc: HDC,
    buttons: &[(Button, Rect)],
    active: Option<Button>,
    current: Color,
) {
    unsafe {
        let bg = CreateSolidBrush(COLORREF(0x00FF_FFFF));
        let bg_active = CreateSolidBrush(COLORREF(0x00F5_E8D6));
        let border = CreatePen(PS_SOLID, 1, COLORREF(0x00A0_A0A0));
        let old_pen = SelectObject(hdc, HGDIOBJ(border.0));
        for (b, r) in buttons {
            let brush = if Some(*b) == active { bg_active } else { bg };
            let old = SelectObject(hdc, HGDIOBJ(brush.0));
            let _ = Rectangle(hdc, r.x, r.y, r.x + r.w, r.y + r.h);
            SelectObject(hdc, old);
            draw_icon(hdc, *b, *r, current);
        }
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ(border.0));
        let _ = DeleteObject(HGDIOBJ(bg.0));
        let _ = DeleteObject(HGDIOBJ(bg_active.0));
    }
}

pub unsafe fn paint_palette(hdc: HDC, swatches: &[(Color, Rect)]) {
    unsafe {
        let border = CreatePen(PS_SOLID, 1, COLORREF(0x0040_4040));
        let old_pen = SelectObject(hdc, HGDIOBJ(border.0));
        for (c, r) in swatches {
            let brush = CreateSolidBrush(COLORREF(c.colorref()));
            let old = SelectObject(hdc, HGDIOBJ(brush.0));
            let _ = Rectangle(hdc, r.x, r.y, r.x + r.w, r.y + r.h);
            SelectObject(hdc, old);
            let _ = DeleteObject(HGDIOBJ(brush.0));
        }
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ(border.0));
    }
}

unsafe fn line(hdc: HDC, a: (i32, i32), b: (i32, i32)) {
    unsafe {
        let _ = MoveToEx(hdc, a.0, a.1, None);
        let _ = LineTo(hdc, b.0, b.1);
    }
}

/// Ícones simples desenhados com primitivas GDI (sem bitmaps externos).
unsafe fn draw_icon(hdc: HDC, b: Button, r: Rect, current: Color) {
    unsafe {
        let cx = r.x + r.w / 2;
        let cy = r.y + r.h / 2;
        let ink = CreatePen(PS_SOLID, 2, COLORREF(0x0030_3030));
        let old_pen = SelectObject(hdc, HGDIOBJ(ink.0));
        let hollow = HGDIOBJ(GetStockObject(NULL_BRUSH).0);
        let old_brush = SelectObject(hdc, hollow);
        let (x0, x1, y0, y1) = (r.x + 8, r.x + r.w - 8, r.y + 8, r.y + r.h - 8);
        match b {
            Button::Pen => {
                let pts = [
                    POINT { x: x0, y: y1 },
                    POINT { x: x0 + 6, y: y0 + 4 },
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
                let _ = Rectangle(hdc, x0, y0 + 3, x1, y1 - 3);
            }
            Button::Ellipse => {
                let _ = Ellipse(hdc, x0, y0 + 3, x1, y1 - 3);
            }
            Button::Marker => {
                draw::fill_alpha(hdc, Rect { x: x0, y: cy - 5, w: x1 - x0, h: 10 }, 0x0000_FFFF, 160);
            }
            Button::Text => {
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x0030_3030));
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
                let _ = Ellipse(hdc, cx - 8, cy - 8, cx + 8, cy + 8);
            }
            Button::ColorSwatch => {
                let brush = CreateSolidBrush(COLORREF(current.colorref()));
                let old = SelectObject(hdc, HGDIOBJ(brush.0));
                let _ = Rectangle(hdc, x0 + 2, y0 + 2, x1 - 2, y1 - 2);
                SelectObject(hdc, old);
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            Button::Undo => {
                line(hdc, (x1, cy), (x0 + 4, cy));
                line(hdc, (x0 + 4, cy), (x0 + 9, cy - 5));
                line(hdc, (x0 + 4, cy), (x0 + 9, cy + 5));
            }
            Button::Copy => {
                let _ = Rectangle(hdc, x0, y0, x0 + 12, y0 + 12);
                let _ = Rectangle(hdc, x0 + 6, y0 + 6, x1, y1);
            }
            Button::Save => {
                let _ = Rectangle(hdc, x0, y0, x1, y1);
                let _ = Rectangle(hdc, x0 + 5, y0, x1 - 5, y0 + 7);
            }
            Button::Close => {
                line(hdc, (x0, y0), (x1, y1));
                line(hdc, (x1, y0), (x0, y1));
            }
        }
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ(ink.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;

    #[test]
    fn vertical_bar_fits_inside_screen() {
        let screen = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        let sel = Rect { x: 1800, y: 900, w: 100, h: 100 };
        let l = layout(sel, screen);
        assert!(l.vertical.right() <= screen.right());
        assert!(l.vertical.bottom() <= screen.bottom());
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
