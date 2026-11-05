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

/// Grava JPEG (qualidade 1..=100, croma 4:2:0) a partir do BGRA composto. É bem mais leve
/// que PNG para capturas de tela. Sem canal alfa: a imagem final é sempre opaca.
pub fn to_jpeg(bgra: &[u8], w: u32, h: u32, quality: u8, path: &Path) -> std::io::Result<()> {
    if w == 0 || h == 0 || w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(std::io::Error::other("dimensões de imagem inválidas para JPEG"));
    }
    let rgb: Vec<u8> = bgra
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect();
    let mut enc = jpeg_encoder::Encoder::new_file(path, quality.clamp(1, 100))
        .map_err(std::io::Error::other)?;
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_2_2);
    enc.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(std::io::Error::other)
}

/// Salva no formato pela extensão do caminho: `.png` mantém PNG (sem perda, maior);
/// qualquer outra coisa vira JPEG, o padrão por ser o mais leve.
pub fn save_image(bgra: &[u8], w: u32, h: u32, quality: u8, path: &Path) -> std::io::Result<()> {
    let is_png = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if is_png {
        to_png(bgra, w, h, path)
    } else {
        to_jpeg(bgra, w, h, quality, path)
    }
}

/// Coloca a imagem na área de transferência como CF_DIB de 24 bits (cola em Word, Paint,
/// Teams). 24 bits em vez de 32 deixa o bitmap 25% menor sem perder compatibilidade.
pub fn copy_to_clipboard(hwnd: HWND, bgra: &[u8], w: i32, h: i32) -> windows::core::Result<()> {
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let src_row = (w * 4) as usize;
    // Linhas de DIB são alinhadas em 4 bytes.
    let row = ((w as usize * 3) + 3) & !3;
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
            biBitCount: 24,
            biCompression: BI_RGB.0,
            biSizeImage: (row * h as usize) as u32,
            ..Default::default()
        };
        std::ptr::copy_nonoverlapping(
            &header as *const BITMAPINFOHEADER as *const u8,
            ptr,
            header_size,
        );
        let dst = ptr.add(header_size);
        std::ptr::write_bytes(dst, 0, row * h as usize);
        for y in 0..h as usize {
            let src = &bgra[(h as usize - 1 - y) * src_row..(h as usize - y) * src_row];
            let out = dst.add(y * row);
            for (x, px) in src.chunks_exact(4).enumerate() {
                // BGRA -> BGR (descarta o alfa).
                std::ptr::copy_nonoverlapping(px.as_ptr(), out.add(x * 3), 3);
            }
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

/// Nome sugerido: Captura_AAAA-MM-DD_HH-MM-SS.jpg
pub fn suggested_name() -> String {
    let t = unsafe { GetLocalTime() };
    format!(
        "Captura_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.jpg",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Abre "Salvar como". Não existe pasta padrão (decisão do PRD).
pub fn save_dialog(hwnd: HWND) -> Option<PathBuf> {
    // JPEG primeiro (padrão, mais leve); PNG continua disponível para quem precisar de
    // imagem sem perda.
    let filter: Vec<u16> = "Imagem JPEG - leve (*.jpg)\0*.jpg\0Imagem PNG (*.png)\0*.png\0\0"
        .encode_utf16()
        .collect();
    let defext = wide_z("jpg");
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

    /// Imagem tipo foto: gradiente com ruído leve (determinístico), onde PNG comprime mal.
    fn photo_like(w: usize, h: usize) -> Vec<u8> {
        let mut seed = 12345u32;
        let mut px = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = ((seed >> 24) & 0x1F) as i32;
                let b = ((x * 255 / w) as i32 + noise).clamp(0, 255) as u8;
                let g = ((y * 255 / h) as i32 + noise).clamp(0, 255) as u8;
                let r = (((x + y) * 255 / (w + h)) as i32 + noise).clamp(0, 255) as u8;
                px.extend_from_slice(&[b, g, r, 255]);
            }
        }
        px
    }

    #[test]
    fn jpeg_is_valid_and_lighter_than_png() {
        let dir = std::env::temp_dir().join("shot_jpeg_test");
        std::fs::create_dir_all(&dir).unwrap();
        let (w, h) = (320usize, 200usize);
        let img = photo_like(w, h);
        let jpg = dir.join("t.jpg");
        let png = dir.join("t.png");
        to_jpeg(&img, w as u32, h as u32, 80, &jpg).unwrap();
        to_png(&img, w as u32, h as u32, &png).unwrap();
        let jb = std::fs::read(&jpg).unwrap();
        assert_eq!(&jb[..2], &[0xFF, 0xD8], "começa com SOI");
        assert_eq!(&jb[jb.len() - 2..], &[0xFF, 0xD9], "termina com EOI");
        let pn = std::fs::metadata(&png).unwrap().len();
        assert!(
            (jb.len() as u64) < pn,
            "JPEG ({} B) deve ser mais leve que PNG ({} B)",
            jb.len(),
            pn
        );
    }

    #[test]
    fn lower_quality_gives_lighter_jpeg() {
        let dir = std::env::temp_dir().join("shot_jpeg_q");
        std::fs::create_dir_all(&dir).unwrap();
        let img = photo_like(320, 200);
        let hi = dir.join("hi.jpg");
        let lo = dir.join("lo.jpg");
        to_jpeg(&img, 320, 200, 95, &hi).unwrap();
        to_jpeg(&img, 320, 200, 40, &lo).unwrap();
        assert!(std::fs::metadata(&lo).unwrap().len() < std::fs::metadata(&hi).unwrap().len());
    }

    #[test]
    fn save_image_picks_format_by_extension() {
        let dir = std::env::temp_dir().join("shot_save_ext");
        std::fs::create_dir_all(&dir).unwrap();
        let img = photo_like(16, 16);
        let (j, p) = (dir.join("a.jpg"), dir.join("b.PNG"));
        save_image(&img, 16, 16, 80, &j).unwrap();
        save_image(&img, 16, 16, 80, &p).unwrap();
        assert_eq!(&std::fs::read(&j).unwrap()[..2], &[0xFF, 0xD8]);
        assert_eq!(&std::fs::read(&p).unwrap()[..4], &[137, 80, 78, 71]);
    }

    #[test]
    fn jpeg_rejects_empty_image() {
        let p = std::env::temp_dir().join("shot_jpeg_empty.jpg");
        assert!(to_jpeg(&[], 0, 0, 80, &p).is_err());
    }

    #[test]
    fn suggested_name_has_jpg_extension() {
        assert!(suggested_name().ends_with(".jpg"));
    }
}
