//! Pixelização irreversível: cada bloco recebe a média dos seus pixels.
//! Os pixels originais são sobrescritos no buffer, então não ficam no arquivo.

use crate::geom::Rect;

/// `buf` é BGRA de 4 bytes por pixel; `stride` em bytes por linha.
/// A região é recortada nos limites da imagem.
pub fn pixelate_region(buf: &mut [u8], stride: usize, img_w: i32, img_h: i32, r: Rect, block: u32) {
    let block = block.max(1) as i32;
    let r = r.clamp_to(Rect { x: 0, y: 0, w: img_w, h: img_h });
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    let mut by = r.y;
    while by < r.bottom() {
        let bh = block.min(r.bottom() - by);
        let mut bx = r.x;
        while bx < r.right() {
            let bw = block.min(r.right() - bx);
            let mut sum = [0u32; 4];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let i = y as usize * stride + x as usize * 4;
                    for c in 0..4 {
                        sum[c] += buf[i + c] as u32;
                    }
                }
            }
            let n = (bw * bh) as u32;
            let avg = [
                (sum[0] / n) as u8,
                (sum[1] / n) as u8,
                (sum[2] / n) as u8,
                (sum[3] / n) as u8,
            ];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let i = y as usize * stride + x as usize * 4;
                    buf[i..i + 4].copy_from_slice(&avg);
                }
            }
            bx += block;
        }
        by += block;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_becomes_average_and_original_gone() {
        // 4x1 BGRA: preto, branco, preto, branco
        let mut buf = vec![
            0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        ];
        pixelate_region(&mut buf, 16, 4, 1, Rect { x: 0, y: 0, w: 4, h: 1 }, 4);
        assert!(buf.chunks(4).all(|p| p[0] == 127 || p[0] == 128));
        assert_ne!(buf[0], 0, "pixel original preto não pode permanecer");
    }

    #[test]
    fn region_clipped_to_image() {
        let mut buf = vec![9u8; 4 * 2 * 2];
        pixelate_region(&mut buf, 8, 2, 2, Rect { x: 1, y: 1, w: 50, h: 50 }, 8);
        assert_eq!(buf[0], 9); // pixel fora da região intacto
    }

    #[test]
    fn empty_region_is_noop() {
        let mut buf = vec![7u8; 16];
        pixelate_region(&mut buf, 8, 2, 2, Rect { x: 5, y: 5, w: 3, h: 3 }, 2);
        assert!(buf.iter().all(|&b| b == 7));
    }
}
