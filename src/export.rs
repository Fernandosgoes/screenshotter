//! Achatamento da imagem final: pixelização (irreversível) e depois formas vetoriais,
//! PNG, área de transferência (CF_DIB) e diálogo "Salvar como" (sem pasta padrão).

use crate::capture::Capture;
use crate::draw;
use crate::geom::Rect;
use crate::model::{Annotation, Document};
use crate::pixelate::pixelate_region;
use std::ffi::c_void;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HGDIOBJ, RGBQUAD, SelectObject,
};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};
use windows::core::PCWSTR;

/// Tamanho do bloco de pixelização, em pixels da imagem.
const PIXEL_BLOCK: u32 = 10;

/// Recorte da seleção (BGRA) com a pixelização já aplicada, sem as formas vetoriais.
/// Devolve a seleção recortada aos limites da captura e os pixels. A tela usa isto para
/// mostrar o borrão ao vivo, e a exportação parte do mesmo resultado.
pub fn crop_pixelated(cap: &Capture, sel: Rect, doc: &Document) -> (Rect, Vec<u8>) {
    let sel = sel.clamp_to(Rect { x: 0, y: 0, w: cap.width, h: cap.height });
    let (w, h) = (sel.w, sel.h);
    let stride = (w * 4) as usize;
    let mut out = vec![0u8; stride * h.max(0) as usize];
    if w <= 0 || h <= 0 {
        return (sel, out);
    }

    // Recorte original.
    let cap_stride = (cap.width * 4) as usize;
    for row in 0..h as usize {
        let src = (sel.y as usize + row) * cap_stride + sel.x as usize * 4;
        out[row * stride..(row + 1) * stride].copy_from_slice(&cap.pixels[src..src + stride]);
    }

    // A pixelização sobrescreve os pixels originais (irreversível).
    for a in doc.items() {
        if let Annotation::Pixelate { r } = a {
            let rel = Rect { x: r.x - sel.x, y: r.y - sel.y, w: r.w, h: r.h };
            pixelate_region(&mut out, stride, w, h, rel, PIXEL_BLOCK);
        }
    }
    (sel, out)
}

/// Imagem final da seleção, BGRA `sel.w × sel.h`, com pixelização e formas aplicadas.
pub fn compose(cap: &Capture, sel: Rect, doc: &Document) -> Vec<u8> {
    let (sel, mut out) = crop_pixelated(cap, sel, doc);
    let (w, h) = (sel.w, sel.h);
    if w <= 0 || h <= 0 {
        return out;
    }

    // Formas vetoriais, desenhadas por GDI num DIB com a mesma imagem.
    draw_vectors(&mut out, w, h, doc.items(), sel);

    // Canal alfa sempre opaco no arquivo final.
    for px in out.chunks_exact_mut(4) {
        px[3] = 255;
    }
    out
}

fn draw_vectors(buf: &mut [u8], w: i32, h: i32, items: &[Annotation], sel: Rect) {
    unsafe {
        let mem = CreateCompatibleDC(None);
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
        if let Ok(bmp) = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(buf.as_ptr(), bits as *mut u8, buf.len());
                let old = SelectObject(mem, HGDIOBJ(bmp.0));
                draw::paint_annotations(mem, items, -sel.x, -sel.y, false);
                std::ptr::copy_nonoverlapping(bits as *const u8, buf.as_mut_ptr(), buf.len());
                SelectObject(mem, old);
            }
            let _ = DeleteObject(HGDIOBJ(bmp.0));
        }
        let _ = DeleteDC(mem);
    }
}

/// Grava PNG (RGBA de 8 bits) a partir do BGRA composto.
pub fn to_png(bgra: &[u8], w: u32, h: u32, path: &Path) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(std::io::Error::other)?;
    let rgba: Vec<u8> = bgra
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0], 255])
        .collect();
    writer.write_image_data(&rgba).map_err(std::io::Error::other)?;
    Ok(())
}

/// Coloca a imagem na área de transferência como CF_DIB (cola em Word, Paint, Teams).
pub fn copy_to_clipboard(hwnd: HWND, bgra: &[u8], w: i32, h: i32) -> windows::core::Result<()> {
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let row = (w * 4) as usize;
    let total = header_size + row * h as usize;
    unsafe {
        let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, total)?;
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let header = BITMAPINFOHEADER {
            biSize: header_size as u32,
            biWidth: w,
            biHeight: h, // bottom-up, formato CF_DIB padrão
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        std::ptr::copy_nonoverlapping(
            &header as *const BITMAPINFOHEADER as *const u8,
            ptr,
            header_size,
        );
        let dst = ptr.add(header_size);
        for y in 0..h as usize {
            let src_row = &bgra[(h as usize - 1 - y) * row..(h as usize - y) * row];
            std::ptr::copy_nonoverlapping(src_row.as_ptr(), dst.add(y * row), row);
        }
        let _ = GlobalUnlock(hmem);

        OpenClipboard(Some(hwnd))?;
        let _ = EmptyClipboard();
        const CF_DIB: u32 = 8;
        let set = SetClipboardData(CF_DIB, Some(HANDLE(hmem.0)));
        let _ = CloseClipboard();
        set.map(|_| ())
    }
}

/// Nome sugerido: Captura_AAAA-MM-DD_HH-MM-SS.png
pub fn suggested_name() -> String {
    let t = unsafe { GetLocalTime() };
    format!(
        "Captura_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.png",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Abre "Salvar como". Não existe pasta padrão (decisão do PRD).
pub fn save_dialog(hwnd: HWND) -> Option<PathBuf> {
    let filter: Vec<u16> = "Imagem PNG (*.png)\0*.png\0\0".encode_utf16().collect();
    let defext = wide_z("png");
    let mut file = vec![0u16; 260];
    let name = wide_z(&suggested_name());
    file[..name.len().min(259)].copy_from_slice(&name[..name.len().min(259)]);
    unsafe {
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: hwnd,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: windows::core::PWSTR(file.as_mut_ptr()),
            nMaxFile: file.len() as u32,
            lpstrDefExt: PCWSTR(defext.as_ptr()),
            Flags: OFN_OVERWRITEPROMPT,
            ..Default::default()
        };
        if GetSaveFileNameW(&mut ofn).as_bool() {
            let end = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            Some(PathBuf::from(String::from_utf16_lossy(&file[..end])))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Color;
    use crate::geom::Point;
    use crate::model::Document;

    #[test]
    fn compose_pixelates_before_export() {
        let mut cap = Capture::empty(4, 1);
        cap.pixels = vec![
            0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        ];
        let mut doc = Document::new();
        doc.push(Annotation::Pixelate { r: Rect { x: 0, y: 0, w: 4, h: 1 } });
        let out = compose(&cap, Rect { x: 0, y: 0, w: 4, h: 1 }, &doc);
        assert_eq!(out.len(), 16);
        assert!(out.chunks(4).all(|p| p[0] == 127 || p[0] == 128));
        assert_eq!(out[3], 255);
    }

    #[test]
    fn crop_pixelated_hides_original_pixels_for_live_preview() {
        // Palavra "sensível": faixa alternando preto e branco, ilegível depois do borrão.
        let mut cap = Capture::empty(20, 1);
        for x in 0..20usize {
            let v = if x % 2 == 0 { 0 } else { 255 };
            cap.pixels[x * 4..x * 4 + 4].copy_from_slice(&[v, v, v, 255]);
        }
        let mut doc = Document::new();
        doc.push(Annotation::Pixelate { r: Rect { x: 0, y: 0, w: 20, h: 1 } });
        let (sel, px) = crop_pixelated(&cap, Rect { x: 0, y: 0, w: 20, h: 1 }, &doc);
        assert_eq!(sel, Rect { x: 0, y: 0, w: 20, h: 1 });
        // Dentro de cada bloco de 10 px todos os pixels ficam iguais (quadriculado).
        assert!(px[..40].chunks(4).all(|p| p[0] == px[0]));
        assert!(px[40..80].chunks(4).all(|p| p[0] == px[40]));
        assert!(px.chunks(4).all(|p| p[0] != 0 && p[0] != 255), "nenhum pixel original resta");
    }

    #[test]
    fn crop_pixelated_without_pixelate_is_plain_crop() {
        let mut cap = Capture::empty(2, 1);
        cap.pixels = vec![1, 2, 3, 255, 9, 8, 7, 255];
        let (_, px) = crop_pixelated(&cap, Rect { x: 1, y: 0, w: 1, h: 1 }, &Document::new());
        assert_eq!(&px[..3], &[9, 8, 7]);
    }

    #[test]
    fn compose_respects_selection_offset() {
        let mut cap = Capture::empty(2, 1);
        cap.pixels = vec![1, 2, 3, 255, 9, 8, 7, 255];
        let doc = Document::new();
        let out = compose(&cap, Rect { x: 1, y: 0, w: 1, h: 1 }, &doc);
        assert_eq!(&out[..3], &[9, 8, 7]);
    }

    #[test]
    fn compose_ignores_number_color_type_usage() {
        // Sanity: uma anotação de número não quebra a composição.
        let cap = Capture::empty(20, 20);
        let mut doc = Document::new();
        doc.push(Annotation::Number { center: Point { x: 10, y: 10 }, n: 1, color: Color::Red });
        let out = compose(&cap, Rect { x: 0, y: 0, w: 20, h: 20 }, &doc);
        assert_eq!(out.len(), 20 * 20 * 4);
    }

    #[test]
    fn png_is_written_with_dimensions() {
        let dir = std::env::temp_dir().join("shot_png_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.png");
        to_png(&[0, 0, 255, 255, 0, 255, 0, 255], 2, 1, &p).unwrap();
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn suggested_name_has_png_extension() {
        assert!(suggested_name().ends_with(".png"));
    }
}
