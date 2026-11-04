//! Captura da área virtual (todos os monitores) para um bitmap BGRA em memória.
//! Nada é gravado em disco aqui.

use crate::geom::Point;
use std::ffi::c_void;
use std::ptr::null_mut;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ, RGBQUAD, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

/// `origin` é a posição da área virtual na área de trabalho (pode ser negativa
/// em monitores à esquerda/acima do principal). `pixels` é BGRA top-down.
#[derive(Clone)]
pub struct Capture {
    pub origin: Point,
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl Capture {
    #[cfg(test)]
    pub fn empty(width: i32, height: i32) -> Self {
        Capture {
            origin: Point { x: 0, y: 0 },
            width,
            height,
            pixels: vec![0; (width.max(0) * height.max(0) * 4) as usize],
        }
    }
}

/// Cópia escurecida (50%) para o fundo da sobreposição.
pub fn dimmed(pixels: &[u8]) -> Vec<u8> {
    pixels.iter().map(|&b| b / 2).collect()
}

pub fn virtual_screen_rect() -> (Point, i32, i32) {
    unsafe {
        (
            Point {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            },
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

pub fn capture_virtual_screen() -> windows::core::Result<Capture> {
    let (origin, w, h) = virtual_screen_rect();
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
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
        let bmp = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let old = SelectObject(mem, HGDIOBJ(bmp.0));
        let blit = BitBlt(mem, 0, 0, w, h, Some(screen), origin.x, origin.y, SRCCOPY);
        let len = (w * h * 4) as usize;
        let pixels = if blit.is_ok() && !bits.is_null() {
            std::slice::from_raw_parts(bits as *const u8, len).to_vec()
        } else {
            vec![0; len]
        };
        SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        blit?;
        Ok(Capture { origin, width: w, height: h, pixels })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimmed_halves_channels() {
        assert_eq!(dimmed(&[200, 100, 50, 255]), vec![100, 50, 25, 127]);
    }

    #[test]
    fn empty_capture_has_right_size() {
        let c = Capture::empty(3, 2);
        assert_eq!(c.pixels.len(), 24);
    }

    #[test]
    #[ignore = "requer sessão de desktop"]
    fn capture_has_expected_size() {
        let c = capture_virtual_screen().unwrap();
        assert!(c.width > 0 && c.height > 0);
        assert_eq!(c.pixels.len(), (c.width * c.height * 4) as usize);
    }
}
