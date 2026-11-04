//! Desenho GDI das anotações. Usado tanto na tela quanto na exportação,
//! então o que se vê é o que se copia.

use crate::config::Color;
use crate::geom::{Point, Rect};
use crate::model::{Annotation, NUMBER_RADIUS, text_line_height};
use std::ffi::c_void;
use std::ptr::null_mut;
use windows::Win32::Foundation::{POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CLEARTYPE_QUALITY,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen, CreateSolidBrush, DIB_RGB_COLORS,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, Ellipse,
    GetStockObject, GetTextExtentPoint32W,
    HDC, HGDIOBJ, LineTo, MoveToEx, NULL_BRUSH, Polyline, PS_SOLID, Rectangle, RGBQUAD,
    SelectObject, SetBkMode, SetTextColor, TextOutW, TRANSPARENT,
};
use windows::core::w;

fn pt(p: Point) -> POINT {
    POINT { x: p.x, y: p.y }
}

/// Seleciona um pincel e uma caneta, executa `f`, e restaura/descarta tudo.
unsafe fn with_pen_brush(
    hdc: HDC,
    color: u32,
    width: i32,
    hollow: bool,
    f: impl FnOnce(),
) {
    unsafe {
        let pen = CreatePen(PS_SOLID, width.max(1), windows::Win32::Foundation::COLORREF(color));
        let brush = if hollow {
            HGDIOBJ(GetStockObject(NULL_BRUSH).0)
        } else {
            HGDIOBJ(CreateSolidBrush(windows::Win32::Foundation::COLORREF(color)).0)
        };
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let old_brush = SelectObject(hdc, brush);
        f();
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        if !hollow {
            let _ = DeleteObject(brush);
        }
    }
}

/// Ponta da seta: duas linhas saindo de `b` em ±26° em relação a `a→b`.
pub fn arrow_head(a: Point, b: Point, len: f64) -> Option<(Point, Point)> {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    let ang = dy.atan2(dx);
    let spread = 0.45_f64;
    let p1 = Point {
        x: (b.x as f64 - len * (ang - spread).cos()).round() as i32,
        y: (b.y as f64 - len * (ang - spread).sin()).round() as i32,
    };
    let p2 = Point {
        x: (b.x as f64 - len * (ang + spread).cos()).round() as i32,
        y: (b.y as f64 - len * (ang + spread).sin()).round() as i32,
    };
    Some((p1, p2))
}

/// Preenche um retângulo com cor semitransparente (marca-texto).
pub unsafe fn fill_alpha(hdc: HDC, r: Rect, color: u32, alpha: u8) {
    unsafe {
        let mem = CreateCompatibleDC(Some(hdc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 1,
                biHeight: -1,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            bmiColors: [RGBQUAD::default()],
        };
        let mut bits: *mut c_void = null_mut();
        if let Ok(bmp) = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            if !bits.is_null() {
                *(bits as *mut u32) = color;
            }
            let old = SelectObject(mem, HGDIOBJ(bmp.0));
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 0,
            };
            let _ = AlphaBlend(hdc, r.x, r.y, r.w, r.h, mem, 0, 0, 1, 1, blend);
            SelectObject(mem, old);
            let _ = DeleteObject(HGDIOBJ(bmp.0));
        }
        let _ = DeleteDC(mem);
    }
}

/// Desenha um objeto. `dx`/`dy` são deslocados (ex.: origem da seleção na exportação).
/// `hint_pixelate` desenha a moldura tracejada da pixelização (só na tela).
pub unsafe fn paint_annotation(hdc: HDC, a: &Annotation, dx: i32, dy: i32, hint_pixelate: bool) {
    let sh = |p: Point| Point { x: p.x + dx, y: p.y + dy };
    let shr = |r: Rect| Rect { x: r.x + dx, y: r.y + dy, w: r.w, h: r.h };
    unsafe {
        match a {
            Annotation::Pen { points, color, width } => {
                let pts: Vec<POINT> = points.iter().map(|p| pt(sh(*p))).collect();
                if pts.len() == 1 {
                    // Um clique só: ponto visível.
                    let p = pts[0];
                    with_pen_brush(hdc, color.colorref(), *width as i32, false, || {
                        let _ = Ellipse(hdc, p.x - 1, p.y - 1, p.x + 1, p.y + 1);
                    });
                } else if pts.len() > 1 {
                    with_pen_brush(hdc, color.colorref(), *width as i32, true, || {
                        let _ = Polyline(hdc, &pts);
                    });
                }
            }
            Annotation::Line { a, b, color, width } => {
                with_pen_brush(hdc, color.colorref(), *width as i32, true, || {
                    let (a, b) = (sh(*a), sh(*b));
                    let _ = MoveToEx(hdc, a.x, a.y, None);
                    let _ = LineTo(hdc, b.x, b.y);
                });
            }
            Annotation::Arrow { a, b, color, width } => {
                let len = 10.0 + 3.0 * *width as f64;
                with_pen_brush(hdc, color.colorref(), *width as i32, true, || {
                    let (a2, b2) = (sh(*a), sh(*b));
                    let _ = MoveToEx(hdc, a2.x, a2.y, None);
                    let _ = LineTo(hdc, b2.x, b2.y);
                    if let Some((p1, p2)) = arrow_head(a2, b2, len) {
                        let _ = MoveToEx(hdc, p1.x, p1.y, None);
                        let _ = LineTo(hdc, b2.x, b2.y);
                        let _ = LineTo(hdc, p2.x, p2.y);
                    }
                });
            }
            Annotation::Rect { r, color, width } => {
                let r = shr(*r);
                with_pen_brush(hdc, color.colorref(), *width as i32, true, || {
                    let _ = Rectangle(hdc, r.x, r.y, r.x + r.w, r.y + r.h);
                });
            }
            Annotation::Ellipse { r, color, width } => {
                let r = shr(*r);
                with_pen_brush(hdc, color.colorref(), *width as i32, true, || {
                    let _ = Ellipse(hdc, r.x, r.y, r.x + r.w, r.y + r.h);
                });
            }
            Annotation::Marker { r, color } => {
                fill_alpha(hdc, shr(*r), color.colorref(), 102);
            }
            Annotation::Text { at, text, color, size } => {
                draw_text(hdc, sh(*at), text, *color, *size);
            }
            Annotation::Pixelate { r } => {
                if hint_pixelate {
                    let r = shr(*r);
                    with_pen_brush(hdc, 0x0080_8080, 1, true, || {
                        let _ = Rectangle(hdc, r.x, r.y, r.x + r.w, r.y + r.h);
                    });
                }
            }
            Annotation::Number { center, n, color } => {
                let c = sh(*center);
                let rr = NUMBER_RADIUS;
                with_pen_brush(hdc, color.colorref(), 2, false, || {
                    let _ = Ellipse(hdc, c.x - rr, c.y - rr, c.x + rr, c.y + rr);
                });
                let font = create_font(16);
                let old_font = SelectObject(hdc, HGDIOBJ(font.0));
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FF_FFFF));
                let mut rc = RECT {
                    left: c.x - rr,
                    top: c.y - rr,
                    right: c.x + rr,
                    bottom: c.y + rr,
                };
                let mut wide: Vec<u16> = n.to_string().encode_utf16().collect();
                DrawTextW(hdc, &mut wide, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                SelectObject(hdc, old_font);
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
    }
}

unsafe fn create_font(size: i32) -> windows::Win32::Graphics::Gdi::HFONT {
    unsafe {
        CreateFontW(
            -size,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
            windows::Win32::Graphics::Gdi::OUT_DEFAULT_PRECIS,
            windows::Win32::Graphics::Gdi::CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        )
    }
}

unsafe fn draw_text(hdc: HDC, at: Point, text: &str, color: Color, size: u32) {
    unsafe {
        let font = create_font(size as i32);
        let old_font = SelectObject(hdc, HGDIOBJ(font.0));
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(color.colorref()));
        SetBkMode(hdc, TRANSPARENT);
        let line_h = text_line_height(size);
        for (i, line) in text.split('\n').enumerate() {
            let wide: Vec<u16> = line.encode_utf16().collect();
            let _ = TextOutW(hdc, at.x, at.y + i as i32 * line_h, &wide);
        }
        SelectObject(hdc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
}

/// Mede o texto com a mesma fonte usada ao desenhar. Devolve (largura da linha mais larga,
/// largura da última linha), em pixels; serve para a caixa de edição e o cursor.
pub unsafe fn measure_text(hdc: HDC, text: &str, size: u32) -> (i32, i32) {
    unsafe {
        let font = create_font(size as i32);
        let old_font = SelectObject(hdc, HGDIOBJ(font.0));
        let width_of = |line: &str| {
            let wide: Vec<u16> = line.encode_utf16().collect();
            let mut sz = SIZE::default();
            if wide.is_empty() || !GetTextExtentPoint32W(hdc, &wide, &mut sz).as_bool() {
                0
            } else {
                sz.cx
            }
        };
        let widest = text.split('\n').map(width_of).max().unwrap_or(0);
        let last = text.rsplit('\n').next().map(width_of).unwrap_or(0);
        SelectObject(hdc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        (widest, last)
    }
}

/// Atalho para pintar uma lista inteira.
pub unsafe fn paint_annotations(hdc: HDC, items: &[Annotation], dx: i32, dy: i32, hint_pixelate: bool) {
    for a in items {
        unsafe { paint_annotation(hdc, a, dx, dy, hint_pixelate) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_head_wings_are_behind_tip() {
        let (p1, p2) = arrow_head(Point { x: 0, y: 0 }, Point { x: 100, y: 0 }, 10.0).unwrap();
        assert!(p1.x < 100 && p2.x < 100, "pontas da seta ficam atrás da ponta");
        assert_ne!(p1.y, p2.y, "asas em lados opostos");
    }

    #[test]
    fn arrow_head_none_for_zero_length() {
        assert!(arrow_head(Point { x: 5, y: 5 }, Point { x: 5, y: 5 }, 10.0).is_none());
    }
}
