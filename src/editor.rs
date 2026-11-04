//! Estado da captura em curso: seleção, ferramenta, cor, espessura, texto.
//! Não toca Win32. Recebe eventos já traduzidos e muda o `Document`.

use crate::capture::Capture;
use crate::config::{Color, Settings};
use crate::geom::{self, Handle, Point, Rect};
use crate::model::{Annotation, Document};

/// Tolerância para pegar uma alça da seleção, em pixels.
pub const HANDLE_TOL: i32 = 6;
const MIN_SEL: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Line,
    Arrow,
    Rect,
    Ellipse,
    Marker,
    Text,
    Pixelate,
    Number,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interaction {
    Idle,
    Selecting { anchor: Point },
    ResizingSel { handle: Handle },
    Drawing { start: Point },
    Moving { last: Point },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextEdit {
    pub at: Point,
    pub buf: String,
}

pub struct Editor {
    pub capture: Capture,
    pub doc: Document,
    /// Seleção em coordenadas de cliente da sobreposição (= coordenadas da captura).
    pub sel: Option<Rect>,
    pub tool: Tool,
    pub color: Color,
    pub stroke: u32,
    pub text_size: u32,
    pub text_background: bool,
    pub palette_open: bool,
    pub state: Interaction,
    pub text: Option<TextEdit>,
    pub shift: bool,
}

impl Editor {
    pub fn new(capture: Capture, settings: &Settings) -> Self {
        Editor {
            capture,
            doc: Document::new(),
            sel: None,
            tool: Tool::Pen,
            color: settings.default_color,
            stroke: settings.stroke_width,
            text_size: settings.text_size,
            text_background: settings.text_background,
            palette_open: false,
            state: Interaction::Idle,
            text: None,
            shift: false,
        }
    }

    #[cfg(test)]
    pub fn test_new(w: i32, h: i32) -> Self {
        Self::new(Capture::empty(w, h), &Settings::default())
    }

    pub fn screen_rect(&self) -> Rect {
        Rect { x: 0, y: 0, w: self.capture.width, h: self.capture.height }
    }

    pub fn set_tool(&mut self, t: Tool) {
        self.commit_text();
        self.tool = t;
        self.doc.clear_selection();
        self.palette_open = false;
    }

    pub fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
    }

    pub fn set_color(&mut self, c: Color) {
        self.color = c;
        self.palette_open = false;
        if let Some(i) = self.doc.selected() {
            recolor(&mut self.doc, i, c);
        }
    }

    /// Roda do mouse: texto (ou ferramenta Texto) muda o tamanho; o resto muda a espessura.
    pub fn on_wheel(&mut self, delta: i16) {
        let up = delta > 0;
        if self.text.is_some() || self.tool == Tool::Text {
            let s = self.text_size as i32 + if up { 2 } else { -2 };
            self.text_size = s.clamp(6, 200) as u32;
        } else {
            let s = self.stroke as i32 + if up { 1 } else { -1 };
            self.stroke = s.clamp(1, 50) as u32;
        }
    }

    pub fn has_selection(&self) -> bool {
        self.sel.is_some()
    }

    pub fn on_mouse_down(&mut self, p: Point) {
        self.palette_open = false;

        // Clicar fora da caixa de texto finaliza o texto.
        if let Some(t) = &self.text {
            let approx = crate::model::bounds_of(&Annotation::Text {
                at: t.at,
                text: t.buf.clone(),
                color: self.color,
                size: self.text_size,
                background: self.text_background,
            });
            if !approx.contains(p) {
                self.commit_text();
            } else {
                return;
            }
        }

        let Some(sel) = self.sel else {
            self.state = Interaction::Selecting { anchor: p };
            self.sel = Some(Rect::from_points(p, p));
            return;
        };

        if let Some(h) = sel.hit_handle(p, HANDLE_TOL) {
            self.state = Interaction::ResizingSel { handle: h };
            return;
        }

        if !sel.contains(p) {
            // Clique fora da seleção: nova seleção.
            self.state = Interaction::Selecting { anchor: p };
            self.sel = Some(Rect::from_points(p, p));
            self.doc.clear_selection();
            return;
        }

        // Dentro da seleção: objeto existente tem prioridade (mover).
        if self.doc.select_at(p).is_some() {
            self.doc.begin_gesture();
            self.state = Interaction::Moving { last: p };
            return;
        }

        match self.tool {
            Tool::Number => {
                let n = self.doc.next_number();
                self.doc.push(Annotation::Number { center: p, n, color: self.color });
                self.state = Interaction::Idle;
            }
            Tool::Text => {
                self.text = Some(TextEdit { at: p, buf: String::new() });
                self.state = Interaction::Idle;
            }
            t => {
                self.doc.clear_selection();
                self.state = Interaction::Drawing { start: p };
                let a = match t {
                    Tool::Pen => Annotation::Pen { points: vec![p], color: self.color, width: self.stroke },
                    Tool::Line => Annotation::Line { a: p, b: p, color: self.color, width: self.stroke },
                    Tool::Arrow => Annotation::Arrow { a: p, b: p, color: self.color, width: self.stroke },
                    Tool::Rect => Annotation::Rect { r: Rect::from_points(p, p), color: self.color, width: self.stroke },
                    Tool::Ellipse => Annotation::Ellipse { r: Rect::from_points(p, p), color: self.color, width: self.stroke },
                    Tool::Marker => Annotation::Marker { r: Rect::from_points(p, p), color: self.color },
                    Tool::Pixelate => Annotation::Pixelate { r: Rect::from_points(p, p) },
                    _ => unreachable!(),
                };
                self.doc.push(a);
            }
        }
    }

    pub fn on_mouse_move(&mut self, p: Point, shift: bool) {
        self.shift = shift;
        match self.state {
            Interaction::Idle => {}
            Interaction::Selecting { anchor } => {
                self.sel = Some(Rect::from_points(anchor, p).clamp_to(self.screen_rect()));
            }
            Interaction::ResizingSel { handle } => {
                if let Some(sel) = self.sel {
                    let r = sel.resize_by_handle(handle, p).clamp_to(self.screen_rect());
                    self.sel = Some(r);
                }
            }
            Interaction::Drawing { start } => {
                let shift_end = |to: Point| if shift { geom::constrain_45(start, to) } else { to };
                let rect = |to: Point| {
                    if shift {
                        geom::square_or_circle(start, to)
                    } else {
                        Rect::from_points(start, to)
                    }
                };
                if let Some(a) = self.doc.last_mut() {
                    match a {
                        Annotation::Pen { points, .. } => points.push(p),
                        Annotation::Line { b, .. } | Annotation::Arrow { b, .. } => *b = shift_end(p),
                        Annotation::Rect { r, .. } | Annotation::Ellipse { r, .. } => *r = rect(p),
                        Annotation::Marker { r, .. } | Annotation::Pixelate { r } => {
                            *r = Rect::from_points(start, p)
                        }
                        _ => {}
                    }
                }
            }
            Interaction::Moving { last } => {
                self.doc.move_selected(p.x - last.x, p.y - last.y);
                self.state = Interaction::Moving { last: p };
            }
        }
    }

    pub fn on_mouse_up(&mut self) {
        match self.state {
            Interaction::Selecting { .. } | Interaction::ResizingSel { .. } => {
                if let Some(sel) = self.sel {
                    if sel.w < MIN_SEL || sel.h < MIN_SEL {
                        self.sel = None;
                    }
                }
            }
            Interaction::Moving { .. } => self.doc.end_gesture(),
            _ => {}
        }
        self.state = Interaction::Idle;
    }

    pub fn commit_text(&mut self) {
        if let Some(t) = self.text.take() {
            if !t.buf.is_empty() {
                self.doc.push(Annotation::Text {
                    at: t.at,
                    text: t.buf,
                    color: self.color,
                    size: self.text_size,
                    background: self.text_background,
                });
            }
        }
    }

    /// Letras de atalho de ferramenta (P L A R E M T B N). Desligadas durante a digitação.
    pub fn on_key_letter(&mut self, c: char) -> bool {
        if self.text.is_some() {
            return false;
        }
        let t = match c.to_ascii_lowercase() {
            'p' => Tool::Pen,
            'l' => Tool::Line,
            'a' => Tool::Arrow,
            'r' => Tool::Rect,
            'e' => Tool::Ellipse,
            'm' => Tool::Marker,
            't' => Tool::Text,
            'b' => Tool::Pixelate,
            'n' => Tool::Number,
            _ => return false,
        };
        self.set_tool(t);
        true
    }

    /// Dígitos 1..6 escolhem a cor (amarelo, azul, verde, vermelho, preto, branco).
    pub fn on_key_digit(&mut self, c: char) -> bool {
        if self.text.is_some() {
            return false;
        }
        match c {
            '1'..='6' => {
                let idx = (c as u8 - b'1') as usize;
                self.set_color(Color::ALL[idx]);
                true
            }
            _ => false,
        }
    }

    pub fn on_char(&mut self, c: char) {
        if let Some(t) = self.text.as_mut() {
            match c {
                '\r' | '\n' => t.buf.push('\n'),
                '\u{8}' => {
                    t.buf.pop();
                }
                c if !c.is_control() => t.buf.push(c),
                _ => {}
            }
        }
    }

    pub fn undo(&mut self) {
        self.commit_text();
        self.doc.undo();
    }

    pub fn redo(&mut self) {
        self.commit_text();
        self.doc.redo();
    }

    pub fn delete_selected(&mut self) {
        self.doc.delete_selected();
    }
}

fn recolor(doc: &mut Document, i: usize, c: Color) {
    doc.set_color_at(i, c);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_creates_selection_and_release_fixes_it() {
        let mut e = Editor::test_new(1920, 1080);
        e.on_mouse_down(Point { x: 10, y: 10 });
        e.on_mouse_move(Point { x: 110, y: 60 }, false);
        assert_eq!(e.sel, Some(Rect { x: 10, y: 10, w: 100, h: 50 }));
        e.on_mouse_up();
        assert_eq!(e.state, Interaction::Idle);
        assert!(e.sel.is_some());
    }

    #[test]
    fn shift_line_is_45_degrees() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Line);
        // Começa no meio: (0,0) cairia na alça NW da seleção.
        e.on_mouse_down(Point { x: 300, y: 300 });
        e.on_mouse_move(Point { x: 400, y: 390 }, true);
        e.on_mouse_up();
        assert!(matches!(e.doc.items()[0], Annotation::Line { b: Point { x: 390, y: 390 }, .. }));
    }

    #[test]
    fn number_tool_increments() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Number);
        e.on_mouse_down(Point { x: 10, y: 10 });
        e.on_mouse_up();
        e.on_mouse_down(Point { x: 50, y: 50 });
        e.on_mouse_up();
        let ns: Vec<u32> = e
            .doc
            .items()
            .iter()
            .filter_map(|a| match a {
                Annotation::Number { n, .. } => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(ns, vec![1, 2]);
    }

    #[test]
    fn wheel_changes_stroke_within_bounds() {
        let mut e = Editor::test_new(800, 600);
        e.stroke = 1;
        e.on_wheel(-120);
        assert_eq!(e.stroke, 1);
        e.on_wheel(120);
        assert_eq!(e.stroke, 2);
    }

    #[test]
    fn text_commits_on_click_outside_and_keeps_text() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Text);
        e.on_mouse_down(Point { x: 20, y: 20 });
        e.on_char('o');
        e.on_char('i');
        e.on_mouse_down(Point { x: 700, y: 500 });
        assert!(matches!(&e.doc.items()[0], Annotation::Text { text, .. } if text == "oi"));
    }

    #[test]
    fn letter_shortcuts_disabled_while_typing() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Text);
        e.on_mouse_down(Point { x: 20, y: 20 });
        assert!(!e.on_key_letter('p'));
        assert_eq!(e.tool, Tool::Text);
    }

    #[test]
    fn digit_selects_color() {
        let mut e = Editor::test_new(800, 600);
        assert!(e.on_key_digit('2'));
        assert_eq!(e.color, Color::Blue);
    }
}
