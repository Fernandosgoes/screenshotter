//! Desenho suavizado (anti-aliased) com GDI+: retângulos arredondados, círculos,
//! linhas e curvas. Usado nas barras de ferramentas e nas alças da seleção.
//! O GDI comum desenha serrilhado; o GDI+ resolve isso sem trazer dependências.

use std::ptr::null_mut;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Graphics::GdiPlus::{
    FillModeAlternate, GdipAddPathArc, GdipClosePathFigure, GdipCreateFromHDC, GdipCreatePath,
    GdipCreatePen1, GdipCreateSolidFill, GdipDeleteBrush, GdipDeleteGraphics, GdipDeletePath,
    GdipDeletePen, GdipDrawEllipse, GdipDrawLine, GdipDrawLines, GdipDrawPath,
    GdipFillEllipse, GdipFillPath, GdipSetPenEndCap, GdipSetPenLineJoin, GdipSetPenStartCap,
    GdipSetPixelOffsetMode, GdipSetSmoothingMode, GdiplusStartup, GdiplusStartupInput,
    GpBrush, GpGraphics, GpPath, GpPen, GpSolidFill, LineCapRound, LineJoinRound,
    PixelOffsetModeHalf, PointF, SmoothingModeAntiAlias, UnitPixel,
};

/// Liga o GDI+ (uma vez, no início do programa). O token fica vivo até o processo acabar.
pub fn startup() {
    let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
    let mut token = 0usize;
    unsafe {
        GdiplusStartup(&mut token, &input, null_mut());
    }
}

/// Monta uma cor ARGB do GDI+ a partir de alfa e 0xRRGGBB.
pub const fn argb(alpha: u8, rgb: u32) -> u32 {
    ((alpha as u32) << 24) | (rgb & 0x00FF_FFFF)
}

/// Converte um COLORREF do Win32 (0x00BBGGRR) em ARGB opaco do GDI+.
pub fn argb_from_colorref(c: u32) -> u32 {
    let (r, g, b) = (c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF);
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// (x, y, largura, altura) em pixels.
pub type RectF = (f32, f32, f32, f32);

pub struct Canvas {
    g: *mut GpGraphics,
}

impl Canvas {
    pub fn from_hdc(hdc: HDC) -> Option<Canvas> {
        let mut g: *mut GpGraphics = null_mut();
        unsafe {
            if GdipCreateFromHDC(hdc, &mut g).0 != 0 || g.is_null() {
                return None;
            }
            GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
            GdipSetPixelOffsetMode(g, PixelOffsetModeHalf);
        }
        Some(Canvas { g })
    }

    unsafe fn pen(&self, color: u32, width: f32) -> *mut GpPen {
        unsafe {
            let mut pen: *mut GpPen = null_mut();
            GdipCreatePen1(color, width, UnitPixel, &mut pen);
            GdipSetPenStartCap(pen, LineCapRound);
            GdipSetPenEndCap(pen, LineCapRound);
            GdipSetPenLineJoin(pen, LineJoinRound);
            pen
        }
    }

    unsafe fn brush(&self, color: u32) -> *mut GpSolidFill {
        unsafe {
            let mut b: *mut GpSolidFill = null_mut();
            GdipCreateSolidFill(color, &mut b);
            b
        }
    }

    unsafe fn round_path(r: RectF, rad: f32) -> *mut GpPath {
        unsafe {
            let (x, y, w, h) = r;
            let rad = rad.min(w / 2.0).min(h / 2.0).max(0.0);
            let d = rad * 2.0;
            let mut p: *mut GpPath = null_mut();
            GdipCreatePath(FillModeAlternate, &mut p);
            GdipAddPathArc(p, x, y, d, d, 180.0, 90.0);
            GdipAddPathArc(p, x + w - d, y, d, d, 270.0, 90.0);
            GdipAddPathArc(p, x + w - d, y + h - d, d, d, 0.0, 90.0);
            GdipAddPathArc(p, x, y + h - d, d, d, 90.0, 90.0);
            GdipClosePathFigure(p);
            p
        }
    }

    pub fn fill_round(&self, r: RectF, radius: f32, color: u32) {
        unsafe {
            let path = Self::round_path(r, radius);
            let b = self.brush(color);
            GdipFillPath(self.g, b as *mut GpBrush, path);
            GdipDeleteBrush(b as *mut GpBrush);
            GdipDeletePath(path);
        }
    }

    pub fn stroke_round(&self, r: RectF, radius: f32, color: u32, width: f32) {
        unsafe {
            let path = Self::round_path(r, radius);
            let pen = self.pen(color, width);
            GdipDrawPath(self.g, pen, path);
            GdipDeletePen(pen);
            GdipDeletePath(path);
        }
    }

    pub fn fill_ellipse(&self, r: RectF, color: u32) {
        unsafe {
            let b = self.brush(color);
            GdipFillEllipse(self.g, b as *mut GpBrush, r.0, r.1, r.2, r.3);
            GdipDeleteBrush(b as *mut GpBrush);
        }
    }

    pub fn stroke_ellipse(&self, r: RectF, color: u32, width: f32) {
        unsafe {
            let pen = self.pen(color, width);
            GdipDrawEllipse(self.g, pen, r.0, r.1, r.2, r.3);
            GdipDeletePen(pen);
        }
    }

    pub fn line(&self, a: (f32, f32), b: (f32, f32), color: u32, width: f32) {
        unsafe {
            let pen = self.pen(color, width);
            GdipDrawLine(self.g, pen, a.0, a.1, b.0, b.1);
            GdipDeletePen(pen);
        }
    }

    fn points(pts: &[(f32, f32)]) -> Vec<PointF> {
        pts.iter().map(|p| PointF { X: p.0, Y: p.1 }).collect()
    }

    /// Linha quebrada passando por todos os pontos.
    pub fn polyline(&self, pts: &[(f32, f32)], color: u32, width: f32) {
        let v = Self::points(pts);
        unsafe {
            let pen = self.pen(color, width);
            GdipDrawLines(self.g, pen, v.as_ptr(), v.len() as i32);
            GdipDeletePen(pen);
        }
    }

    /// Sombra suave embaixo de um painel arredondado (camadas translúcidas).
    pub fn shadow(&self, r: RectF, radius: f32) {
        let (x, y, w, h) = r;
        for i in 0..7 {
            let g = i as f32;
            self.fill_round((x - g, y - g + 3.0, w + 2.0 * g, h + 2.0 * g), radius + g, argb(9, 0x000000));
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteGraphics(self.g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_packs_alpha_and_rgb() {
        assert_eq!(argb(0x80, 0x112233), 0x8011_2233);
    }

    #[test]
    fn colorref_converts_bgr_to_argb() {
        // COLORREF vermelho puro (0x0000FF) vira ARGB vermelho opaco.
        assert_eq!(argb_from_colorref(0x0000_00FF), 0xFFFF_0000);
        // Azul puro COLORREF (0xFF0000) vira ARGB azul opaco.
        assert_eq!(argb_from_colorref(0x00FF_0000), 0xFF00_00FF);
    }
}
