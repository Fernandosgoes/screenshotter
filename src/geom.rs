//! Geometria pura: retângulos, alças, restrição de ângulo e layout das barras.
//! Sem Win32, testável em qualquer host.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Distância em pixels entre a seleção e as barras de ícones (PRD F7).
pub const GAP: i32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl Rect {
    pub fn from_points(a: Point, b: Point) -> Rect {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Rect {
            x,
            y,
            w: (a.x - b.x).abs(),
            h: (a.y - b.y).abs(),
        }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// Recorta o retângulo para caber dentro de `bounds`.
    pub fn clamp_to(&self, bounds: Rect) -> Rect {
        let x0 = self.x.max(bounds.x);
        let y0 = self.y.max(bounds.y);
        let x1 = self.right().min(bounds.right());
        let y1 = self.bottom().min(bounds.bottom());
        Rect {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0),
            h: (y1 - y0).max(0),
        }
    }

    pub fn handles(&self) -> [(Handle, Point); 8] {
        let cx = self.x + self.w / 2;
        let cy = self.y + self.h / 2;
        let r = self.right();
        let b = self.bottom();
        [
            (Handle::NW, Point { x: self.x, y: self.y }),
            (Handle::N, Point { x: cx, y: self.y }),
            (Handle::NE, Point { x: r, y: self.y }),
            (Handle::E, Point { x: r, y: cy }),
            (Handle::SE, Point { x: r, y: b }),
            (Handle::S, Point { x: cx, y: b }),
            (Handle::SW, Point { x: self.x, y: b }),
            (Handle::W, Point { x: self.x, y: cy }),
        ]
    }

    pub fn hit_handle(&self, p: Point, tol: i32) -> Option<Handle> {
        self.handles()
            .into_iter()
            .find(|(_, hp)| (hp.x - p.x).abs() <= tol && (hp.y - p.y).abs() <= tol)
            .map(|(h, _)| h)
    }

    /// Move só as bordas ligadas à alça; a borda oposta fica parada.
    pub fn resize_by_handle(&self, h: Handle, p: Point) -> Rect {
        let mut left = self.x;
        let mut top = self.y;
        let mut right = self.right();
        let mut bottom = self.bottom();
        match h {
            Handle::NW => {
                left = p.x;
                top = p.y;
            }
            Handle::N => top = p.y,
            Handle::NE => {
                right = p.x;
                top = p.y;
            }
            Handle::E => right = p.x,
            Handle::SE => {
                right = p.x;
                bottom = p.y;
            }
            Handle::S => bottom = p.y,
            Handle::SW => {
                left = p.x;
                bottom = p.y;
            }
            Handle::W => left = p.x,
        }
        Rect::from_points(Point { x: left, y: top }, Point { x: right, y: bottom })
    }
}

/// Restrição de linha: horizontal ou vertical quando um eixo domina (ax ≥ 2·ay ou ay ≥ 2·ax),
/// senão diagonal de 45° com o menor dos dois deslocamentos.
pub fn constrain_45(from: Point, to: Point) -> Point {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let ax = dx.abs();
    let ay = dy.abs();
    let sx = if dx < 0 { -1 } else { 1 };
    let sy = if dy < 0 { -1 } else { 1 };
    if ax >= 2 * ay {
        Point { x: from.x + dx, y: from.y }
    } else if ay >= 2 * ax {
        Point { x: from.x, y: from.y + dy }
    } else {
        let m = ax.min(ay);
        Point {
            x: from.x + sx * m,
            y: from.y + sy * m,
        }
    }
}

/// Retângulo com lado igual ao maior deslocamento, mantendo o sentido do arraste.
pub fn square_or_circle(from: Point, to: Point) -> Rect {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let side = dx.abs().max(dy.abs());
    let sx = if dx < 0 { -1 } else { 1 };
    let sy = if dy < 0 { -1 } else { 1 };
    Rect::from_points(
        from,
        Point {
            x: from.x + sx * side,
            y: from.y + sy * side,
        },
    )
}

/// Etiqueta de medidas exibida sobre a seleção, ex.: "206x143".
pub fn dimension_label(r: Rect) -> String {
    format!("{}x{}", r.w, r.h)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarLayout {
    pub vertical: Rect,
    pub horizontal: Rect,
}

/// Posiciona as barras (PRD F7):
/// - vertical: encostada no lado direito da seleção, alinhada embaixo;
/// - horizontal: encostada embaixo da seleção, alinhada à direita.
/// Se não couber fora da seleção dentro da tela, passa para dentro da seleção.
/// Sempre recortada para ficar inteiramente dentro de `screen`.
pub fn layout_bars(sel: Rect, screen: Rect, v_items: i32, h_items: i32, item: i32) -> BarLayout {
    let vw = item;
    let vh = v_items * item;
    let hw = h_items * item;
    let hh = item;

    let mut vx = sel.right() + GAP;
    if vx + vw > screen.right() {
        vx = sel.right() - GAP - vw;
    }
    let vy = sel.bottom() - vh;

    let hx = sel.right() - hw;
    let mut hy = sel.bottom() + GAP;
    if hy + hh > screen.bottom() {
        hy = sel.bottom() - GAP - hh;
    }

    BarLayout {
        vertical: shift_inside(Rect { x: vx, y: vy, w: vw, h: vh }, screen),
        horizontal: shift_inside(Rect { x: hx, y: hy, w: hw, h: hh }, screen),
    }
}

/// Desloca `r` para dentro de `bounds` sem mudar o tamanho (quando couber).
/// Diferente de `clamp_to`, que encolhe e cortaria os botões da barra.
pub fn shift_inside(r: Rect, bounds: Rect) -> Rect {
    let x = r.x.clamp(bounds.x, (bounds.right() - r.w).max(bounds.x));
    let y = r.y.clamp(bounds.y, (bounds.bottom() - r.h).max(bounds.y));
    Rect { x, y, ..r }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_points_normalizes_reverse_drag() {
        let r = Rect::from_points(Point { x: 50, y: 40 }, Point { x: 10, y: 20 });
        assert_eq!(r, Rect { x: 10, y: 20, w: 40, h: 20 });
    }

    #[test]
    fn dimension_label_matches_prd_example() {
        assert_eq!(dimension_label(Rect { x: 0, y: 0, w: 206, h: 143 }), "206x143");
    }

    #[test]
    fn constrain_45_snaps_to_diagonal() {
        let p = constrain_45(Point { x: 0, y: 0 }, Point { x: 100, y: 90 });
        assert_eq!(p, Point { x: 90, y: 90 });
    }

    #[test]
    fn constrain_45_snaps_to_horizontal_when_axis_dominates() {
        let p = constrain_45(Point { x: 0, y: 0 }, Point { x: 100, y: 30 });
        assert_eq!(p, Point { x: 100, y: 0 });
    }

    #[test]
    fn square_uses_larger_delta() {
        let r = square_or_circle(Point { x: 0, y: 0 }, Point { x: 30, y: 10 });
        assert_eq!(r, Rect { x: 0, y: 0, w: 30, h: 30 });
    }

    #[test]
    fn handle_hit_and_resize() {
        let r = Rect { x: 100, y: 100, w: 200, h: 100 };
        assert_eq!(r.hit_handle(Point { x: 101, y: 99 }, 4), Some(Handle::NW));
        assert_eq!(r.hit_handle(Point { x: 200, y: 150 }, 4), None);
        let r2 = r.resize_by_handle(Handle::E, Point { x: 350, y: 0 });
        assert_eq!(r2, Rect { x: 100, y: 100, w: 250, h: 100 });
    }

    #[test]
    fn bars_flip_inside_when_touching_right_edge() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 600, y: 100, w: 400, h: 300 };
        let l = layout_bars(sel, screen, 11, 3, 40);
        assert!(l.vertical.right() <= sel.right(), "barra deve ficar dentro da seleção");
        assert!(l.vertical.x >= sel.x);
    }

    #[test]
    fn bars_outside_when_room() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 100, y: 100, w: 200, h: 200 };
        let l = layout_bars(sel, screen, 11, 3, 40);
        assert_eq!(l.vertical.x, 300 + GAP);
        assert_eq!(l.horizontal.y, 300 + GAP);
    }

    #[test]
    fn bars_never_leave_screen() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 0, y: 760, w: 1000, h: 40 };
        let l = layout_bars(sel, screen, 11, 3, 40);
        assert!(screen.clamp_to(screen) == screen);
        assert_eq!(l.vertical.clamp_to(screen), l.vertical);
        assert_eq!(l.horizontal.clamp_to(screen), l.horizontal);
    }
}
