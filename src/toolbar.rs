//! Barras de ícones (PRD F7) e paleta de 6 cores (F11).
//! Visual: painéis escuros compactos e arredondados com sombra e borda sutil, botões de
//! 30 px, destaque em indigo no botão ativo e realce no hover. Ícones com traço arredondado.
//! Layout puro + pintura suavizada (GDI+, ver gfx.rs).

use crate::config::Color;
use crate::geom::{BarLayout, GAP, Point, Rect, layout_bars, shift_inside};
use crate::gfx::{Canvas, RectF, argb, argb_from_colorref};
use std::f32::consts::{FRAC_PI_2, PI};

/// Tamanho de um botão, em pixels (compacto, parecido com o do Lightshot).
pub const BTN: i32 = 30;
/// Margem interna do painel em volta dos botões.
pub const PAD: i32 = 4;
const SWATCH: i32 = 16;
const SWATCH_GAP: i32 = 4;
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
    Button::ClearAll,
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
    // Mesmo tamanho dos botões da barra, na mesma linha do T.
    let size = BTN;
    let y = vertical.y + PAD + row * BTN;
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
    panel
}

/// Fundo do ícone de mover texto: mesmo tamanho e cantos dos botões da barra, com borda clara.
pub fn paint_flyout(cv: &Canvas, r: Rect) {
    let rr = rf(r);
    cv.fill_round(rr, BTN_RADIUS, argb(0xFF, PANEL_RGB));
    cv.stroke_round((rr.0 + 0.5, rr.1 + 0.5, rr.2 - 1.0, rr.3 - 1.0), BTN_RADIUS - 0.5, argb(0xFF, BORDER_RGB), 1.0);
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
    // Faixa entre a barra e a cruzinha: conta como o T, para a cruzinha não sumir no caminho.
    let tm = l.text_move;
    if tm.x > l.vertical.right() && p.x >= l.vertical.right() && p.x < tm.x && p.y >= tm.y && p.y < tm.y + tm.h {
        return Some(Button::Text);
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

/// Ícones da mockup: desenhados numa grade de 24 unidades, traço suave e pontas arredondadas.
/// A grade é escalada para ~60% do botão.
fn cubic(c: [(f32, f32); 4]) -> Vec<(f32, f32)> {
    (0..=8)
        .map(|i| {
            let t = i as f32 / 8.0;
            let u = 1.0 - t;
            let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (
                a * c[0].0 + b * c[1].0 + cc * c[2].0 + d * c[3].0,
                a * c[0].1 + b * c[1].1 + cc * c[2].1 + d * c[3].1,
            )
        })
        .collect()
}

/// Pontos de um arco de círculo entre os ângulos `a0` e `a1` (radianos).
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * i as f32 / n as f32;
            (cx + r * t.cos(), cy + r * t.sin())
        })
        .collect()
}

/// Ícones da mockup: desenhados numa grade de 24 unidades, traço suave e pontas arredondadas.
/// A grade é escalada para ~60% do botão.

fn draw_icon(cv: &Canvas, b: Button, r: Rect, current: Color, active: bool) {
    let cx = r.x as f32 + r.w as f32 / 2.0;
    let cy = r.y as f32 + r.h as f32 / 2.0;
    // Mover texto é a cruz menor (0,8 da escala da barra), para ficar com a altura do T.
    let k = if b == Button::TextMove { 0.8 } else { 1.0 };
    let sc = BTN as f32 * 0.6 / 24.0 * k;
    let rgb = if active { INK_ACTIVE_RGB } else { INK_RGB };
    let ink = argb(0xFF, rgb);
    let w = (1.5 * sc * 1.25).max(1.1);
    let p = |x: f32, y: f32| (cx + (x - 12.0) * sc, cy + (y - 12.0) * sc);
    // Retângulo (x, y, largura, altura) na grade, convertido para pixels.
    let q = |x: f32, y: f32, ww: f32, hh: f32| {
        let (px, py) = p(x, y);
        (px, py, ww * sc, hh * sc)
    };
    let m = |v: &[(f32, f32)]| -> Vec<(f32, f32)> { v.iter().map(|&(x, y)| p(x, y)).collect() };
    match b {
        Button::Move | Button::TextMove => {
            cv.line(p(12.0, 2.0), p(12.0, 22.0), ink, w);
            cv.line(p(2.0, 12.0), p(22.0, 12.0), ink, w);
            cv.polyline(&[p(9.0, 5.0), p(12.0, 2.0), p(15.0, 5.0)], ink, w);
            cv.polyline(&[p(9.0, 19.0), p(12.0, 22.0), p(15.0, 19.0)], ink, w);
            cv.polyline(&[p(5.0, 9.0), p(2.0, 12.0), p(5.0, 15.0)], ink, w);
            cv.polyline(&[p(19.0, 9.0), p(22.0, 12.0), p(19.0, 15.0)], ink, w);
        }
        Button::Pen => {
            let mut wave = cubic([(3.0, 14.0), (5.0, 10.0), (7.0, 10.0), (9.0, 14.0)]);
            wave.extend(cubic([(9.0, 14.0), (11.0, 18.0), (13.0, 18.0), (15.0, 14.0)]));
            wave.extend(cubic([(15.0, 14.0), (17.0, 10.0), (18.0, 11.0), (21.0, 13.0)]));
            cv.polyline(&m(&wave), ink, w);
        }
        Button::Line => cv.line(p(5.0, 19.0), p(19.0, 5.0), ink, w),
        Button::Arrow => {
            cv.line(p(5.0, 19.0), p(19.0, 5.0), ink, w);
            cv.polyline(&[p(9.0, 5.0), p(19.0, 5.0), p(19.0, 15.0)], ink, w);
        }
        Button::Rect => cv.stroke_round(q(3.0, 5.0, 18.0, 14.0), 3.0 * sc, ink, w),
        Button::Ellipse => cv.stroke_ellipse(q(3.0, 5.0, 18.0, 14.0), ink, w),
        Button::Marker => {
            // Amarelo vivo com contorno âmbar, para a faixa aparecer no fundo claro e no escuro.
            let bar = q(3.5, 9.0, 17.0, 6.0);
            cv.fill_round(bar, 2.0 * sc, argb(0xFF, 0xFFD000));
            cv.stroke_round(bar, 2.0 * sc, argb(0xFF, 0xB8860B), (1.0 * sc).max(1.0));
        }
        Button::Text => {
            cv.polyline(&[p(5.0, 6.0), p(5.0, 4.0), p(19.0, 4.0), p(19.0, 6.0)], ink, w);
            cv.line(p(12.0, 4.0), p(12.0, 20.0), ink, w);
            cv.line(p(9.0, 20.0), p(15.0, 20.0), ink, w);
        }
        Button::Pixelate => {
            for gy in 0..3 {
                for gx in 0..3 {
                    let alpha = if (gx + gy) % 2 == 0 { 0xFF } else { 0x66 };
                    let sq = q(4.0 + gx as f32 * 6.0, 4.0 + gy as f32 * 6.0, 4.0, 4.0);
                    cv.fill_round(sq, 1.0 * sc, argb(alpha, rgb));
                }
            }
        }
        Button::Number => {
            // Círculo grande e um "1" menor, com folga dentro do círculo.
            cv.stroke_ellipse(q(2.0, 2.0, 20.0, 20.0), ink, w);
            cv.line(p(12.0, 8.0), p(12.0, 16.0), ink, w);
            cv.line(p(10.0, 9.8), p(12.0, 8.3), ink, w);
        }
        Button::ColorSwatch => {
            let dot = q(4.5, 4.5, 15.0, 15.0);
            cv.fill_ellipse(dot, argb_from_colorref(current.colorref()));
            cv.stroke_ellipse(dot, argb(0xFF, 0x000000), 1.2);
        }
        Button::Undo => {
            cv.polyline(&[p(9.0, 14.0), p(4.0, 9.0), p(9.0, 4.0)], ink, w);
            cv.line(p(4.0, 9.0), p(14.0, 9.0), ink, w);
            let mut hook = arc(14.0, 15.0, 6.0, -FRAC_PI_2, FRAC_PI_2, 8);
            hook.push((11.0, 21.0));
            cv.polyline(&m(&hook), ink, w);
        }
        Button::ClearAll => {
            // Borracha da mockup: contorno da peça e traço de "apagar" embaixo.
            cv.polyline(&m(&[(3.0, 17.0), (11.0, 9.0), (17.0, 15.0), (13.0, 19.0), (7.0, 19.0), (3.0, 17.0)]), ink, w);
            cv.line(p(14.0, 21.0), p(21.0, 21.0), ink, w);
        }
        Button::Copy => {
            // Duas folhas: a da frente fechada e a de trás só nas bordas visíveis.
            cv.stroke_round(q(8.0, 8.0, 13.0, 13.0), 2.0 * sc, ink, w);
            let mut back = vec![(4.0, 16.0), (4.0, 6.0)];
            back.extend(arc(6.0, 6.0, 2.0, PI, 1.5 * PI, 4));
            back.push((16.0, 4.0));
            cv.polyline(&m(&back), ink, w);
        }
        Button::Save => {
            // Seta descendo para uma bandeja, como o ícone de salvar da marca.
            cv.polyline(&m(&[(4.0, 15.0), (4.0, 19.0), (20.0, 19.0), (20.0, 15.0)]), ink, w);
            cv.line(p(12.0, 3.0), p(12.0, 15.0), ink, w);
            cv.polyline(&m(&[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]), ink, w);
        }
        Button::Close => {
            cv.line(p(6.0, 6.0), p(18.0, 18.0), ink, w);
            cv.line(p(18.0, 6.0), p(6.0, 18.0), ink, w);
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
        let (b, r) = v[4];
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
