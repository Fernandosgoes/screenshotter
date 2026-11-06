//! Modelo de anotações: lista de objetos editáveis com desfazer/refazer.
//! Puro (sem Win32). Cada gesto do usuário vira um passo de desfazer.
// Seleção e movimento de objetos ficam no modelo (com testes), mas o editor não os usa:
// formas desenhadas não podem ser movidas, para permitir desenhar por cima delas.
#![allow(dead_code)]

use crate::config::Color;
use crate::geom::{Point, Rect};

/// Raio do círculo numerado, em pixels.
pub const NUMBER_RADIUS: i32 = 12;
/// Tolerância de clique para selecionar um objeto fino (linhas, traços).
const HIT_TOL: i32 = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Pen { points: Vec<Point>, color: Color, width: u32 },
    Line { a: Point, b: Point, color: Color, width: u32 },
    Arrow { a: Point, b: Point, color: Color, width: u32 },
    Rect { r: Rect, color: Color, width: u32 },
    Ellipse { r: Rect, color: Color, width: u32 },
    Marker { r: Rect, color: Color },
    Text { at: Point, text: String, color: Color, size: u32 },
    Pixelate { r: Rect },
    Number { center: Point, n: u32, color: Color },
}

/// Altura de uma linha de texto para o tamanho de fonte dado (usada ao desenhar e medir).
pub fn text_line_height(size: u32) -> i32 {
    size as i32 * 12 / 10 + 2
}

/// Limites aproximados do objeto, usados para seleção e pixelização.
pub fn bounds_of(a: &Annotation) -> Rect {
    match a {
        Annotation::Pen { points, .. } => {
            if points.is_empty() {
                return Rect { x: 0, y: 0, w: 0, h: 0 };
            }
            let minx = points.iter().map(|p| p.x).min().unwrap();
            let maxx = points.iter().map(|p| p.x).max().unwrap();
            let miny = points.iter().map(|p| p.y).min().unwrap();
            let maxy = points.iter().map(|p| p.y).max().unwrap();
            Rect { x: minx, y: miny, w: maxx - minx, h: maxy - miny }
        }
        Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. } => Rect::from_points(*a, *b),
        Annotation::Rect { r, .. }
        | Annotation::Ellipse { r, .. }
        | Annotation::Marker { r, .. }
        | Annotation::Pixelate { r } => *r,
        Annotation::Text { at, text, size, .. } => {
            let lines = text.split('\n').count() as i32;
            let longest = text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0) as i32;
            let size = *size as i32;
            // Mesma caixa do balão desenhado: texto com folga, sem passar do tamanho real.
            // A largura é uma estimativa acima da média dos caracteres, para o clamp não deixar
            // o balão sair da seleção.
            let line_h = text_line_height(size as u32);
            let pad = crate::editor::TEXT_PAD;
            Rect {
                x: at.x - pad,
                y: at.y - pad,
                w: (longest * size * 3 / 4 + 2 * pad + 4).max(crate::editor::TEXT_MIN_W),
                h: (lines * line_h + 2 * pad).max(line_h + 2 * pad),
            }
        }
        Annotation::Number { center, .. } => Rect {
            x: center.x - NUMBER_RADIUS,
            y: center.y - NUMBER_RADIUS,
            w: NUMBER_RADIUS * 2,
            h: NUMBER_RADIUS * 2,
        },
    }
}

fn translate(a: &mut Annotation, dx: i32, dy: i32) {
    let mv = |p: &mut Point| {
        p.x += dx;
        p.y += dy;
    };
    let mvr = |r: &mut Rect| {
        r.x += dx;
        r.y += dy;
    };
    match a {
        Annotation::Pen { points, .. } => points.iter_mut().for_each(mv),
        Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. } => {
            mv(a);
            mv(b);
        }
        Annotation::Rect { r, .. }
        | Annotation::Ellipse { r, .. }
        | Annotation::Marker { r, .. }
        | Annotation::Pixelate { r } => mvr(r),
        Annotation::Text { at, .. } => mv(at),
        Annotation::Number { center, .. } => mv(center),
    }
}

#[derive(Debug, Default)]
pub struct Document {
    items: Vec<Annotation>,
    selected: Option<usize>,
    undo: Vec<Vec<Annotation>>,
    redo: Vec<Vec<Annotation>>,
    gesture: bool,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[Annotation] {
        &self.items
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn last_mut(&mut self) -> Option<&mut Annotation> {
        self.items.last_mut()
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.items.clone());
        self.redo.clear();
    }

    /// Adiciona um objeto como um passo de desfazer.
    pub fn push(&mut self, a: Annotation) {
        self.checkpoint();
        self.items.push(a);
    }

    /// Inicia um gesto (ex.: arrastar um objeto); todo o gesto vira um único passo.
    pub fn begin_gesture(&mut self) {
        if !self.gesture {
            self.checkpoint();
            self.gesture = true;
        }
    }

    pub fn end_gesture(&mut self) {
        self.gesture = false;
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                let current = std::mem::replace(&mut self.items, prev);
                self.redo.push(current);
                self.selected = None;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                let current = std::mem::replace(&mut self.items, next);
                self.undo.push(current);
                self.selected = None;
                true
            }
            None => false,
        }
    }

    /// Seleciona o objeto mais recente sob o ponto (topo da pilha de desenho).
    pub fn select_at(&mut self, p: Point) -> Option<usize> {
        let hit = self.items.iter().enumerate().rev().find(|(_, a)| {
            let b = bounds_of(a);
            Rect {
                x: b.x - HIT_TOL,
                y: b.y - HIT_TOL,
                w: b.w + 2 * HIT_TOL,
                h: b.h + 2 * HIT_TOL,
            }
            .contains(p)
        });
        self.selected = hit.map(|(i, _)| i);
        self.selected
    }

    /// Seleciona o texto mais recente sob o ponto (ignora formas e números).
    pub fn select_text_at(&mut self, p: Point) -> bool {
        let hit = self.items.iter().enumerate().rev().find(|(_, a)| {
            if !matches!(a, Annotation::Text { .. }) {
                return false;
            }
            let b = bounds_of(a);
            Rect {
                x: b.x - HIT_TOL,
                y: b.y - HIT_TOL,
                w: b.w + 2 * HIT_TOL,
                h: b.h + 2 * HIT_TOL,
            }
            .contains(p)
        });
        self.selected = hit.map(|(i, _)| i);
        self.selected.is_some()
    }

    /// Canto do texto selecionado (se o objeto selecionado for um texto).
    pub fn selected_text_origin(&self) -> Option<Point> {
        match self.selected.and_then(|i| self.items.get(i)) {
            Some(Annotation::Text { at, .. }) => Some(*at),
            _ => None,
        }
    }

    /// Põe o texto selecionado com o canto em `to`, mantendo-o inteiro dentro de `area`.
    pub fn place_selected_text(&mut self, to: Point, area: Rect) {
        let Some(i) = self.selected else { return };
        let Some(a) = self.items.get(i) else { return };
        let Annotation::Text { at, .. } = a else { return };
        let b = bounds_of(a);
        let (ox, oy) = (b.x - at.x, b.y - at.y);
        // Limites do canto: a caixa do texto inteira precisa caber na área.
        let fit = |v: i32, lo: i32, hi: i32| if hi < lo { lo } else { v.clamp(lo, hi) };
        let x = fit(to.x, area.x - ox, area.right() - b.w - ox);
        let y = fit(to.y, area.y - oy, area.bottom() - b.h - oy);
        if let Some(Annotation::Text { at, .. }) = self.items.get_mut(i) {
            *at = Point { x, y };
        }
    }

    /// Apaga todas as anotações de uma vez (um único passo de desfazer).
    pub fn clear_all(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.checkpoint();
        self.items.clear();
        self.selected = None;
        true
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Remove o objeto selecionado e renumera os passos restantes (1, 2, 3…).
    pub fn delete_selected(&mut self) -> bool {
        match self.selected {
            Some(i) if i < self.items.len() => {
                self.checkpoint();
                self.items.remove(i);
                self.renumber();
                self.selected = None;
                true
            }
            _ => false,
        }
    }

    fn renumber(&mut self) {
        let mut next = 1;
        for a in self.items.iter_mut() {
            if let Annotation::Number { n, .. } = a {
                *n = next;
                next += 1;
            }
        }
    }

    /// Move o objeto selecionado. Não grava passo: use begin_gesture/end_gesture ao redor.
    pub fn move_selected(&mut self, dx: i32, dy: i32) {
        if let Some(i) = self.selected {
            if let Some(a) = self.items.get_mut(i) {
                translate(a, dx, dy);
            }
        }
    }

    /// Troca a cor de um objeto (passo de desfazer próprio).
    pub fn set_color_at(&mut self, i: usize, c: Color) {
        if i >= self.items.len() {
            return;
        }
        self.checkpoint();
        let a = &mut self.items[i];
        match a {
            Annotation::Pen { color, .. }
            | Annotation::Line { color, .. }
            | Annotation::Arrow { color, .. }
            | Annotation::Rect { color, .. }
            | Annotation::Ellipse { color, .. }
            | Annotation::Marker { color, .. }
            | Annotation::Text { color, .. }
            | Annotation::Number { color, .. } => *color = c,
            Annotation::Pixelate { .. } => {}
        }
    }

    pub fn next_number(&self) -> u32 {
        self.items
            .iter()
            .filter_map(|a| match a {
                Annotation::Number { n, .. } => Some(*n),
                _ => None,
            })
            .max()
            .map_or(1, |m| m + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(n: u32, x: i32) -> Annotation {
        Annotation::Number { center: Point { x, y: 0 }, n, color: Color::Red }
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut d = Document::new();
        d.push(num(1, 0));
        d.push(num(2, 100));
        assert_eq!(d.items().len(), 2);
        assert!(d.undo());
        assert_eq!(d.items().len(), 1);
        assert!(d.redo());
        assert_eq!(d.items().len(), 2);
    }

    #[test]
    fn push_clears_redo() {
        let mut d = Document::new();
        d.push(num(1, 0));
        d.undo();
        d.push(num(1, 5));
        assert!(!d.redo());
    }

    #[test]
    fn deleting_number_renumbers_rest() {
        let mut d = Document::new();
        d.push(num(1, 0));
        d.push(num(2, 100));
        d.push(num(3, 200));
        d.select_at(Point { x: 0, y: 0 });
        assert!(d.delete_selected());
        let ns: Vec<u32> = d
            .items()
            .iter()
            .filter_map(|a| match a {
                Annotation::Number { n, .. } => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(ns, vec![1, 2]);
        assert_eq!(d.next_number(), 3);
    }

    #[test]
    fn move_gesture_is_one_undo_step() {
        let mut d = Document::new();
        d.push(num(1, 0));
        d.select_at(Point { x: 0, y: 0 });
        d.begin_gesture();
        d.move_selected(5, 0);
        d.move_selected(5, 0);
        d.end_gesture();
        assert!(matches!(d.items()[0], Annotation::Number { center: Point { x: 10, .. }, .. }));
        assert!(d.undo());
        assert!(matches!(d.items()[0], Annotation::Number { center: Point { x: 0, .. }, .. }));
    }

    #[test]
    fn select_prefers_topmost() {
        let mut d = Document::new();
        d.push(num(1, 0));
        d.push(num(2, 0));
        assert_eq!(d.select_at(Point { x: 0, y: 0 }), Some(1));
    }
}
