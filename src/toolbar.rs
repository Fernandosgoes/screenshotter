//! Barras de ícones (PRD F7) e paleta de 6 cores (F11).
//! Visual: painéis escuros compactos e arredondados com sombra e borda sutil, botões de
//! 30 px, destaque em indigo no botão ativo e realce no hover. Ícones com traço arredondado.
//! Layout puro + pintura suavizada (GDI+, ver gfx.rs).

use crate::config::Color;
use crate::geom::{BarLayout, GAP, Point, Rect, layout_bars, shift_inside};
use crate::gfx::{Canvas, RectF, argb, argb_from_colorref};

/// Tamanho de um botão, em pixels (compacto, parecido com o do Lightshot).
pub const BTN: i32 = 30;
/// Margem interna do painel em volta dos botões.
pub const PAD: i32 = 4;
const SWATCH: i32 = 20;
const SWATCH_GAP: i32 = 5;
/// Raio dos cantos do painel e dos botões.
const PANEL_RADIUS: f32 = 12.0;
const BTN_RADIUS: f32 = 8.0;

// Cores em 0xRRGGBB (o alfa é aplicado em cada uso).
const PANEL_RGB: u32 = 0xFFFFFF; // painel branco
const ACCENT_RGB: u32 = 0x272C38; // grafite: botão ativo
const INK_RGB: u32 = 0x171A24; // ícones em quase preto
const INK_ACTIVE_RGB: u32 = 0xFFFFFF; // ícone no botão ativo
const HOVER_RGB: u32 = 0xECEEF2; // cinza do hover
const BORDER_RGB: u32 = 0xD9DCE4; // borda do painel

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Move,
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
    /// Borracha: apaga todas as anotações (ação, não ferramenta).
    ClearAll,
    Copy,
    Save,
    Close,
    /// Ícone de mover texto, que aparece ao lado do T (não faz parte das barras).
    TextMove,
}

pub const VERTICAL: [Button; 13] = [
    Button::Move,
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
    Button::ClearAll,
];

pub const HORIZONTAL: [Button; 3] = [Button::Copy, Button::Save, Button::Close];

fn pad(r: Rect) -> Rect {
    Rect { x: r.x - PAD, y: r.y - PAD, w: r.w + 2 * PAD, h: r.h + 2 * PAD }
}

/// Painéis (com margem) das duas barras, sempre inteiramente dentro de `screen`.
pub fn layout(sel: Rect, screen: Rect) -> BarLayout {
    let base = layout_bars(sel, screen, VERTICAL.len() as i32, HORIZONTAL.len() as i32, BTN);
    let vertical = shift_inside(pad(base.vertical), screen);
    // Ícone de mover texto: do lado de fora da barra, na linha do T; do lado de dentro só se
    // não couber na tela.
    let row = VERTICAL.iter().position(|b| *b == Button::Text).unwrap_or(7) as i32;
    let size = BTN + 2 * PAD;
    let y = vertical.y + row * BTN;
    let outside_x = vertical.right() + GAP;
    let x = if outside_x + size <= screen.right() { outside_x } else { (vertical.x - GAP - size).max(0) };
    BarLayout {
        vertical,
        horizontal: shift_inside(pad(base.horizontal), screen),
        text_move: Rect { x, y, w: size, h: size },
    }
}

/// Retângulo do botão de mover texto, dentro do seu painel.
pub fn text_move_button(panel: Rect) -> Rect {
    Rect { x: panel.x + PAD, y: panel.y + PAD, w: BTN, h: BTN }
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
    if l.text_move.contains(p) {
        return Some(Button::TextMove);
    }
    let (v, h) = button_rects(l);
    v.into_iter().chain(h).find(|(_, r)| r.contains(p)).map(|(b, _)| b)
}

/// Painel da paleta: abre à esquerda da barra vertical, na linha do quadradinho de cor.
pub fn palette_panel(l: &BarLayout, colors: &[Color]) -> Rect {
    let row = VERTICAL.iter().position(|b| *b == Button::ColorSwatch).unwrap_or(9) as i32;
    let w = colors.len() as i32 * (SWATCH + SWATCH_GAP) - SWATCH_GAP + 2 * PAD;
    let h = SWATCH + 2 * PAD;
    let center_y = l.vertical.y + PAD + row * BTN + BTN / 2;
    Rect { x: (l.vertical.x - GAP - w).max(0), y: center_y - h / 2, w, h }
}

pub fn palette_rects(l: &BarLayout, colors: &[Color]) -> Vec<(Color, Rect)> {
    let p = palette_panel(l, colors);
    colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (*c, Rect { x: p.x + PAD + i as i32 * (SWATCH + SWATCH_GAP), y: p.y + PAD, w: SWATCH, h: SWATCH })
        })
        .collect()
}

pub fn palette_hit(l: &BarLayout, colors: &[Color], p: Point) -> Option<Color> {
    palette_rects(l, colors).into_iter().find(|(_, r)| r.contains(p)).map(|(c, _)| c)
}

fn rf(r: Rect) -> RectF {
    (r.x as f32, r.y as f32, r.w as f32, r.h as f32)
}

/// Fundo do painel: sombra suave, preenchimento escuro quase opaco e borda clara sutil.
pub fn paint_panel(cv: &Canvas, panel: Rect) {
    let r = rf(panel);
    cv.shadow(r, PANEL_RADIUS);
    cv.fill_round(r, PANEL_RADIUS, argb(0xFF, PANEL_RGB));
    cv.stroke_round((r.0 + 0.5, r.1 + 0.5, r.2 - 1.0, r.3 - 1.0), PANEL_RADIUS - 0.5, argb(0xFF, BORDER_RGB), 1.0);
}

/// Pinta os botões de uma barra: destaque no ativo, realce no hover, ícones.
pub fn paint_buttons(
    cv: &Canvas,
    buttons: &[(Button, Rect)],
    active: Option<Button>,
    hover: Option<Button>,
    current: Color,
) {
    for (b, r) in buttons {
        let is_active = Some(*b) == active;
        let rr = rf(*r);
        let inner = (rr.0 + 1.0, rr.1 + 1.0, rr.2 - 2.0, rr.3 - 2.0);
        if is_active {
            cv.fill_round(inner, BTN_RADIUS, argb(0xFF, ACCENT_RGB));
        } else if Some(*b) == hover {
            cv.fill_round(inner, BTN_RADIUS, argb(0xFF, HOVER_RGB));
        }
        draw_icon(cv, *b, *r, current, is_active);
    }
}

/// Quadradinho com X para cancelar a captura (canto superior direito da seleção).
pub fn paint_cancel(cv: &Canvas, r: Rect) {
    let rr = rf(r);
    cv.shadow(rr, 4.0);
    cv.fill_round(rr, 4.0, argb(0xFF, PANEL_RGB));
    cv.stroke_round((rr.0 + 0.5, rr.1 + 0.5, rr.2 - 1.0, rr.3 - 1.0), 3.5, argb(0xFF, BORDER_RGB), 1.0);
    let m = 5.0;
    let (x0, y0) = (rr.0 + m, rr.1 + m);
    let (x1, y1) = (rr.0 + rr.2 - m, rr.1 + rr.3 - m);
    cv.line((x0, y0), (x1, y1), argb(0xFF, INK_RGB), 1.8);
    cv.line((x1, y0), (x0, y1), argb(0xFF, INK_RGB), 1.8);
}

/// Paleta: bolinhas com a cor; a cor ativa tem anel de destaque.
pub fn paint_palette(cv: &Canvas, panel: Rect, swatches: &[(Color, Rect)], current: Color) {
    paint_panel(cv, panel);
    for (c, r) in swatches {
        let rr = rf(*r);
        cv.fill_ellipse(rr, argb_from_colorref(c.colorref()));
        // Contorno de contraste: preto ganha branco por dentro; as outras cores ganham preto.
        let outline = if *c == Color::Black { 0xFFFFFF } else { 0x000000 };
        cv.stroke_ellipse(rr, argb(0xFF, outline), 1.2);
        if *c == current {
            cv.stroke_ellipse((rr.0 - 3.5, rr.1 - 3.5, rr.2 + 7.0, rr.3 + 7.0), argb(0xFF, ACCENT_RGB), 2.0);
        }
    }
}

/// Ícones desenhados em vetor, com pontas redondas. As coordenadas abaixo foram
/// desenhadas para um botão de 40 px e são escaladas para o tamanho real do botão.
fn draw_icon(cv: &Canvas, b: Button, r: Rect, current: Color, active: bool) {
    let cx = r.x as f32 + r.w as f32 / 2.0;
    let cy = r.y as f32 + r.h as f32 / 2.0;
    let s = BTN as f32 / 40.0;
    let rgb = if active { INK_ACTIVE_RGB } else { INK_RGB };
    let ink = argb(0xFF, rgb);
    let w = (2.2 * s).max(1.7);
    let p = |dx: f32, dy: f32| (cx + dx * s, cy + dy * s);
    // Retângulo (x, y, largura, altura) relativo ao centro do botão.
    let q = |x: f32, y: f32, ww: f32, hh: f32| (cx + x * s, cy + y * s, ww * s, hh * s);
    match b {
        Button::Move | Button::TextMove => {
            // Cruz de quatro setas.
            cv.line(p(-8.0, 0.0), p(8.0, 0.0), ink, w);
            cv.line(p(0.0, -8.0), p(0.0, 8.0), ink, w);
            cv.polyline(&[p(-3.0, -5.0), p(0.0, -8.0), p(3.0, -5.0)], ink, w);
            cv.polyline(&[p(-3.0, 5.0), p(0.0, 8.0), p(3.0, 5.0)], ink, w);
            cv.polyline(&[p(-5.0, -3.0), p(-8.0, 0.0), p(-5.0, 3.0)], ink, w);
            cv.polyline(&[p(5.0, -3.0), p(8.0, 0.0), p(5.0, 3.0)], ink, w);
        }
        Button::Pen => cv.curve(&[p(-8.0, 3.0), p(-3.0, -5.0), p(2.0, 4.0), p(8.0, -4.0)], ink, w),
        Button::Line => cv.line(p(-7.0, 7.0), p(7.0, -7.0), ink, w),
        Button::Arrow => {
            cv.line(p(-7.0, 7.0), p(7.0, -7.0), ink, w);
            cv.line(p(7.0, -7.0), p(-0.5, -7.0), ink, w);
            cv.line(p(7.0, -7.0), p(7.0, 0.5), ink, w);
        }
        Button::Rect => cv.stroke_round(q(-7.5, -6.0, 15.0, 12.0), 3.0 * s, ink, w),
        Button::Ellipse => cv.stroke_ellipse(q(-8.0, -6.0, 16.0, 12.0), ink, w),
        Button::Marker => cv.fill_round(q(-8.0, -3.0, 16.0, 8.0), 2.5 * s, argb(0xDD, 0xFACC15)),
        Button::Text => {
            cv.line(p(-6.5, -6.0), p(6.5, -6.0), ink, w);
            cv.line(p(0.0, -6.0), p(0.0, 7.0), ink, w);
        }
        Button::Pixelate => {
            for gy in 0..3 {
                for gx in 0..3 {
                    let alpha = if (gx + gy) % 2 == 0 { 0xFF } else { 0x66 };
                    let sq = q(-9.5 + gx as f32 * 7.0, -9.5 + gy as f32 * 7.0, 5.0, 5.0);
                    cv.fill_round(sq, 1.2 * s, argb(alpha, rgb));
                }
            }
        }
        Button::Number => {
            cv.stroke_ellipse(q(-9.0, -9.0, 18.0, 18.0), ink, w);
            cv.line(p(-1.5, -3.5), p(1.0, -5.5), ink, w);
            cv.line(p(1.0, -5.5), p(1.0, 5.0), ink, w);
        }
        Button::ColorSwatch => {
            let rr = q(-9.0, -9.0, 18.0, 18.0);
            cv.fill_ellipse(rr, argb_from_colorref(current.colorref()));
            let outline = if current == Color::Black { 0xFFFFFF } else { 0x000000 };
            cv.stroke_ellipse(rr, argb(0xFF, outline), 1.4);
        }
        Button::Undo => {
            cv.curve(&[p(-8.0, 0.0), p(-1.0, -2.0), p(6.0, 1.0), p(7.0, 6.0)], ink, w);
            cv.line(p(-8.0, 0.0), p(-3.0, -5.0), ink, w);
            cv.line(p(-8.0, 0.0), p(-3.0, 5.0), ink, w);
        }
        Button::ClearAll => {
            // Borracha: corpo arredondado com divisória e um traço de "apagar" embaixo.
            cv.stroke_round(q(-8.0, -7.0, 16.0, 10.0), 3.0 * s, ink, w);
            cv.line(p(0.0, -7.0), p(0.0, 3.0), ink, w);
            cv.line(p(-6.0, 8.0), p(8.0, 8.0), ink, w);
        }
        Button::Copy => {
            // Folha de trás (só a parte visível) e folha da frente.
            cv.polyline(&[p(-3.0, 3.0), p(-8.0, 3.0), p(-8.0, -8.0), p(3.0, -8.0), p(3.0, -3.0)], ink, w);
            cv.stroke_round(q(-3.0, -3.0, 11.0, 11.0), 2.5 * s, ink, w);
        }
        Button::Save => {
            cv.stroke_round(q(-7.5, -7.5, 15.0, 15.0), 3.0 * s, ink, w);
            cv.stroke_round(q(-3.5, -7.5, 7.0, 5.0), 1.5 * s, ink, w * 0.85);
            cv.stroke_round(q(-4.5, 1.0, 9.0, 6.5), 1.5 * s, ink, w * 0.85);
        }
        Button::Close => {
            cv.line(p(-6.0, -6.0), p(6.0, 6.0), ink, w);
            cv.line(p(6.0, -6.0), p(-6.0, 6.0), ink, w);
        }
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
        let (b, r) = v[3];
        assert_eq!(hit(&l, Point { x: r.x + 1, y: r.y + 1 }), Some(b));
        assert_eq!(b, Button::Arrow);
    }

    #[test]
    fn palette_hit_maps_six_colors() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 300, y: 300, w: 200, h: 200 };
        let l = layout(sel, screen);
        let rects = palette_rects(&l, &Color::ALL);
        assert_eq!(rects.len(), 6);
        for (c, r) in rects {
            assert_eq!(palette_hit(&l, &Color::ALL, Point { x: r.x + 2, y: r.y + 2 }), Some(c));
        }
    }
}
