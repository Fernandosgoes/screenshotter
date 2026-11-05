//! Estado da captura em curso: seleção, ferramenta, cor, espessura, texto.
//! Não toca Win32. Recebe eventos já traduzidos e muda o `Document`.

use crate::capture::Capture;
use crate::config::{Color, Settings};
use crate::geom::{self, Handle, Point, Rect};
use crate::model::{Annotation, Document, bounds_of, text_line_height};

/// Tolerância para pegar uma alça da seleção, em pixels.
pub const HANDLE_TOL: i32 = 6;
const MIN_SEL: i32 = 2;
const NUMBER_COLORS: [Color; 3] = [Color::Red, Color::Green, Color::Blue];
/// Lado do quadradinho com X (cancelar) no canto superior direito da seleção.
pub const CANCEL_SIZE: i32 = 18;
/// Distância do quadradinho X até as bordas da seleção.
const CANCEL_INSET: i32 = 4;
/// Margem entre o texto e a borda da caixa de edição.
pub const TEXT_PAD: i32 = 5;
/// Largura mínima da caixa de edição de texto (visível mesmo vazia).
pub const TEXT_MIN_W: i32 = 120;

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
    /// Cruz: arrastar dentro da seleção move a área (só com esta ferramenta).
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interaction {
    Idle,
    Selecting { anchor: Point },
    ResizingSel { handle: Handle },
    /// Movendo a seleção: `origin` é a seleção no início do arraste.
    Moving { anchor: Point, origin: Rect },
    Drawing { start: Point },
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
    /// Qualidade do JPEG ao salvar (vem das configurações).
    pub jpeg_quality: u8,
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
            jpeg_quality: settings.jpeg_quality,
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
        // Contador só usa vermelho/verde (branco some no fundo claro).
        if !self.palette_colors().contains(&self.color) {
            self.color = NUMBER_COLORS[0];
        }
    }

    /// Cores oferecidas pela paleta: o contador só aceita vermelho e verde.
    pub fn palette_colors(&self) -> &'static [Color] {
        if self.tool == Tool::Number { &NUMBER_COLORS } else { &Color::ALL }
    }

    pub fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
    }

    pub fn set_color(&mut self, c: Color) {
        if !self.palette_colors().contains(&c) {
            return;
        }
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

    /// As barras de ferramentas só aparecem com a seleção pronta (como no Lightshot):
    /// ficam escondidas enquanto a área está sendo arrastada ou redimensionada.
    pub fn toolbar_visible(&self) -> bool {
        self.sel.is_some()
            && !matches!(
                self.state,
                Interaction::Selecting { .. }
                    | Interaction::ResizingSel { .. }
                    | Interaction::Moving { .. }
            )
    }

    /// Quadradinho X de cancelar: canto superior direito, por dentro da seleção.
    pub fn cancel_rect(&self) -> Option<Rect> {
        if !self.toolbar_visible() {
            return None;
        }
        let sel = self.sel?;
        Some(Rect {
            x: sel.right() - CANCEL_INSET - CANCEL_SIZE,
            y: sel.y + CANCEL_INSET,
            w: CANCEL_SIZE,
            h: CANCEL_SIZE,
        })
    }

    pub fn cancel_hit(&self, p: Point) -> bool {
        self.cancel_rect().is_some_and(|r| r.contains(p))
    }

    /// Caixa de edição do texto (com margem), visível mesmo com o texto vazio.
    pub fn text_box(&self) -> Option<Rect> {
        let t = self.text.as_ref()?;
        let b = bounds_of(&Annotation::Text {
            at: t.at,
            text: t.buf.clone(),
            color: self.color,
            size: self.text_size,
        });
        let min_h = text_line_height(self.text_size);
        Some(Rect {
            x: t.at.x - TEXT_PAD,
            y: t.at.y - TEXT_PAD,
            w: (b.w + 2 * TEXT_PAD).max(TEXT_MIN_W),
            h: (b.h + 2 * TEXT_PAD).max(min_h + 2 * TEXT_PAD),
        })
    }

    pub fn on_mouse_down(&mut self, p: Point) {
        self.palette_open = false;

        // Clicar fora da caixa de texto finaliza o texto.
        if let Some(boxr) = self.text_box() {
            if boxr.contains(p) {
                return;
            }
            self.commit_text();
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
            // Clique fora da seleção não faz nada: a área só sai pelo X, pelo ESC
            // ou depois de cancelar, quando dá para desenhar uma nova.
            return;
        }

        // Dentro da seleção, o clique usa a ferramenta atual: formas já desenhadas
        // não são selecionadas nem movidas, assim dá para desenhar por cima e dentro delas.
        match self.tool {
            Tool::Move => {
                self.state = Interaction::Moving { anchor: p, origin: sel };
            }
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
            Interaction::Moving { anchor, origin } => {
                let screen = self.screen_rect();
                let x = (origin.x + p.x - anchor.x).clamp(0, (screen.w - origin.w).max(0));
                let y = (origin.y + p.y - anchor.y).clamp(0, (screen.h - origin.h).max(0));
                self.sel = Some(Rect { x, y, ..origin });
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
                });
            }
        }
    }

    /// Enter na caixa de texto: finaliza e cola o texto. Shift+Enter quebra a linha.
    /// Retorna true se havia uma caixa de texto aberta (a tecla foi consumida).
    pub fn on_enter(&mut self, shift: bool) -> bool {
        match self.text.as_mut() {
            None => false,
            Some(t) => {
                if shift {
                    t.buf.push('\n');
                } else {
                    self.commit_text();
                }
                true
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
                match self.palette_colors().get(idx) {
                    Some(&col) => {
                        self.set_color(col);
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }

    pub fn on_char(&mut self, c: char) {
        if let Some(t) = self.text.as_mut() {
            // Enter/Shift+Enter vêm por on_enter (precisa saber se o Shift está apertado).
            match c {
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

    /// Borracha: apaga todos os desenhos, mantendo a seleção. Desfazível com Ctrl+Z.
    pub fn clear_all(&mut self) {
        self.commit_text();
        self.doc.clear_all();
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
    fn drawing_inside_an_existing_shape_does_not_move_it() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Rect);
        e.on_mouse_down(Point { x: 100, y: 100 });
        e.on_mouse_move(Point { x: 400, y: 400 }, false);
        e.on_mouse_up();
        // Clique no meio do retângulo com outra ferramenta: desenha, não move.
        e.set_tool(Tool::Line);
        e.on_mouse_down(Point { x: 200, y: 200 });
        assert!(matches!(e.state, Interaction::Drawing { .. }));
        e.on_mouse_move(Point { x: 300, y: 250 }, false);
        e.on_mouse_up();
        assert_eq!(e.doc.items().len(), 2);
        assert!(matches!(e.doc.items()[0], Annotation::Rect { r: Rect { x: 100, y: 100, w: 300, h: 300 }, .. }));
        // Mesma ferramenta (retângulo) sobre a borda de outro retângulo também cria um novo.
        e.set_tool(Tool::Rect);
        e.on_mouse_down(Point { x: 100, y: 200 });
        e.on_mouse_move(Point { x: 150, y: 260 }, false);
        e.on_mouse_up();
        assert_eq!(e.doc.items().len(), 3);
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
    fn enter_commits_text_and_closes_box() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Text);
        e.on_mouse_down(Point { x: 20, y: 20 });
        e.on_char('o');
        e.on_char('i');
        assert!(e.on_enter(false));
        assert!(e.text.is_none());
        assert!(matches!(&e.doc.items()[0], Annotation::Text { text, .. } if text == "oi"));
    }

    #[test]
    fn shift_enter_adds_line_break_and_keeps_editing() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Text);
        e.on_mouse_down(Point { x: 20, y: 20 });
        e.on_char('a');
        assert!(e.on_enter(true));
        e.on_char('b');
        assert!(e.text.is_some());
        e.on_enter(false);
        assert!(matches!(&e.doc.items()[0], Annotation::Text { text, .. } if text == "a\nb"));
    }

    #[test]
    fn enter_without_text_box_is_not_consumed() {
        let mut e = Editor::test_new(800, 600);
        assert!(!e.on_enter(false));
        assert!(!e.on_enter(true));
    }

    #[test]
    fn empty_text_box_is_visible_and_click_inside_keeps_it() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Text);
        e.on_mouse_down(Point { x: 100, y: 100 });
        let b = e.text_box().expect("caixa aberta");
        assert!(b.w >= 120 && b.h > 0);
        e.on_mouse_down(Point { x: 150, y: 110 }); // dentro da caixa
        assert!(e.text.is_some());
    }

    #[test]
    fn toolbar_hidden_while_selecting_and_shown_after_release() {
        let mut e = Editor::test_new(800, 600);
        assert!(!e.toolbar_visible());
        e.on_mouse_down(Point { x: 10, y: 10 });
        e.on_mouse_move(Point { x: 200, y: 150 }, false);
        assert!(!e.toolbar_visible(), "barra escondida durante o arraste");
        e.on_mouse_up();
        assert!(e.toolbar_visible(), "barra aparece ao soltar o mouse");
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

    #[test]
    fn number_tool_only_red_green_blue() {
        let mut e = Editor::test_new(100, 100);
        e.set_color(Color::White);
        e.set_tool(Tool::Number);
        assert_eq!(e.color, Color::Red);
        assert_eq!(e.palette_colors().len(), 3);
        e.set_color(Color::Yellow);
        assert_eq!(e.color, Color::Red);
        e.set_color(Color::Blue);
        assert_eq!(e.color, Color::Blue);
        e.set_color(Color::Green);
        assert_eq!(e.color, Color::Green);
        e.set_tool(Tool::Pen);
        assert_eq!(e.palette_colors().len(), 6);
    }

    #[test]
    fn clear_all_removes_everything_and_keeps_selection_and_undoes() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 0, y: 0, w: 800, h: 600 });
        e.set_tool(Tool::Rect);
        e.on_mouse_down(Point { x: 100, y: 100 });
        e.on_mouse_move(Point { x: 200, y: 200 }, false);
        e.on_mouse_up();
        e.set_tool(Tool::Number);
        e.on_mouse_down(Point { x: 300, y: 300 });
        e.on_mouse_up();
        assert_eq!(e.doc.items().len(), 2);
        e.clear_all();
        assert!(e.doc.items().is_empty());
        assert!(e.sel.is_some(), "a seleção continua");
        e.undo();
        assert_eq!(e.doc.items().len(), 2, "Ctrl+Z desfaz o clear all");
    }

    #[test]
    fn click_outside_selection_keeps_it() {
        let mut e = Editor::test_new(800, 600);
        e.on_mouse_down(Point { x: 100, y: 100 });
        e.on_mouse_move(Point { x: 300, y: 300 }, false);
        e.on_mouse_up();
        let sel = e.sel;
        e.on_mouse_down(Point { x: 600, y: 500 });
        e.on_mouse_up();
        assert_eq!(e.sel, sel, "clique fora não cancela a área");
    }

    #[test]
    fn cancel_button_hit_test() {
        let mut e = Editor::test_new(800, 600);
        e.on_mouse_down(Point { x: 100, y: 100 });
        e.on_mouse_move(Point { x: 300, y: 300 }, false);
        e.on_mouse_up();
        let cr = e.cancel_rect().expect("X visível com a barra");
        assert!(e.cancel_hit(Point { x: cr.x + 2, y: cr.y + 2 }));
        assert!(!e.cancel_hit(Point { x: 150, y: 150 }));
    }

    #[test]
    fn move_tool_moves_selection_only_when_active() {
        let mut e = Editor::test_new(800, 600);
        e.sel = Some(Rect { x: 100, y: 100, w: 200, h: 100 });
        // Com caneta, arrastar dentro da área desenha e não move.
        e.on_mouse_down(Point { x: 150, y: 150 });
        e.on_mouse_move(Point { x: 180, y: 180 }, false);
        e.on_mouse_up();
        assert_eq!(e.sel, Some(Rect { x: 100, y: 100, w: 200, h: 100 }));
        // Com a cruz, arrastar move a área.
        e.set_tool(Tool::Move);
        e.on_mouse_down(Point { x: 150, y: 150 });
        e.on_mouse_move(Point { x: 180, y: 190 }, false);
        e.on_mouse_up();
        assert_eq!(e.sel, Some(Rect { x: 130, y: 140, w: 200, h: 100 }));
        // Não sai da tela.
        e.on_mouse_down(Point { x: 200, y: 200 });
        e.on_mouse_move(Point { x: 5000, y: 5000 }, false);
        e.on_mouse_up();
        assert_eq!(e.sel, Some(Rect { x: 600, y: 500, w: 200, h: 100 }));
    }
}
