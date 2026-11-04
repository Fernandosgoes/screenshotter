# Screenshotter local (Windows) v1 — Plano de Implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans para implementar task a task. Os passos usam checkbox (`- [ ]`) para acompanhamento.

**Goal:** Capturador de tela Windows de arquivo único (`.exe`) que replica o fluxo do Lightshot (atalho → seleção → anotação → copiar/salvar), sem nenhuma conexão de rede.

**Architecture:** Processo residente na bandeja com uma janela de sobreposição topmost pré-criada e oculta. No atalho, a tela inteira (todos os monitores) é copiada para um bitmap em memória e mostrada na sobreposição, escurecida. A seleção é uma máscara sobre essa cópia. Anotações são uma lista de objetos vetoriais editáveis; a imagem final é achatada só na exportação (pixelização aplicada sobre os pixels originais, depois as formas são desenhadas por cima).

**Tech Stack:** Rust 2024 (rustc 1.99.1, toolchain MSVC), crate `windows` 0.62 (APIs Win32/GDI), `png` 0.18 (encoder PNG), `serde`/`serde_json` (configurações). Sem crates de rede.

## Global Constraints

- Zero rede: nenhuma crate de rede em `Cargo.toml` (`reqwest`, `hyper`, `ureq`, `tokio`, `winsock` etc. são proibidos). Verificado na Task 14.
- Um arquivo `.exe` único, sem instalador, sem runtime externo.
- Sem telemetria, sem logs com conteúdo de imagem, sem arquivos temporários. A imagem só vai para disco com Ctrl+S.
- Sem upload, sem nuvem, sem contas. Fora do escopo da v1.
- Metas (propostas, medir depois): atalho → seleção ≤ 100 ms; Ctrl+C → clipboard ≤ 200 ms; RAM na bandeja ≤ 30 MB; `.exe` ≤ 10 MB.
- Textos de interface em português; identificadores de código em inglês.
- Caminho do arquivo de configuração: `settings.json` ao lado do `.exe`.
- Aplicação é DPI-aware por monitor (PerMonitorV2) antes de qualquer janela ser criada.

## Desvios documentados em relação ao PRD

- **Desenho:** GDI com double buffering em vez de Direct2D. Motivo: a cena é pequena (retângulos, linhas, texto); GDI atende as metas com muito menos código e sem fábrica COM. Revisitar só se a Task 13 medir estouro de 16 ms por quadro.
- **Exportação PNG:** crate `png` em vez de WIC. Motivo: menos código unsafe/COM. Os pixels e o formato são os mesmos.
- **Configurações (F22):** Task 12 entrega janela simples com controles Win32 padrão. Se a janela ficar fora do prazo, o arquivo JSON continua válido como fallback.

## File Structure

- `Cargo.toml` — manifest, dependências, perfil release (`opt-level = 3`, `lto = true`, `strip = true`, `panic = "abort"`).
- `src/main.rs` — ponto de entrada: DPI, carrega config, cria app, roda o loop de mensagens.
- `src/config.rs` — `Settings` (tipo puro), `load/save` JSON, `default`. Puro, testável.
- `src/geom.rs` — `Rect`, `Point`, normalização, alças, hit-test de alças, rótulo de medidas, constrição de 45°, layout das barras. Puro, testável.
- `src/model.rs` — `Annotation`, `Tool`, `Document` (lista, seleção, desfazer/refazer, renumeração). Puro, testável.
- `src/pixelate.rs` — pixelização de um buffer BGRA por blocos. Puro, testável.
- `src/capture.rs` — captura da área virtual (todos os monitores) para `Capture { width, height, origin, pixels: Vec<u8> }`.
- `src/export.rs` — `compose(capture, selection, doc) -> Vec<u8>` (BGRA da seleção, com pixelização e formas), `to_png`, `copy_to_clipboard`.
- `src/tray.rs` — ícone da bandeja, menu de contexto (Sair, Configurações).
- `src/hotkey.rs` — `RegisterHotKey` com PrtScn e fallback Shift+PrtScn.
- `src/overlay.rs` — janela topmost, pintura double-buffered, roteamento de mouse e teclado para o estado do editor.
- `src/editor.rs` — estado da captura em curso: seleção, ferramenta ativa, cor, espessura, edição de texto, barras. Não toca Win32 diretamente (recebe eventos, devolve comandos).
- `src/toolbar.rs` — layout e pintura das barras vertical e horizontal, paleta de cores.
- `src/startup.rs` — `Run` key do registro (iniciar com Windows).
- `src/settings_window.rs` — janela de configurações.
- `src/log.rs` — log de latência no `stderr` apenas com `--debug`.

---

### Task 1: Configurações (config.rs)

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` (adicionar `mod config;`)

**Interfaces:**
- Produces: `pub struct Settings { pub hotkey: HotkeyChoice, pub start_with_windows: bool, pub stroke_width: u32, pub text_size: u32, pub default_color: Color, pub text_background: bool }`; `pub enum HotkeyChoice { PrtScn, ShiftPrtScn }`; `pub enum Color { Yellow, Blue, Green, Red, Black, White }`; `Settings::load(path: &Path) -> Settings` (nunca falha: arquivo ausente ou inválido → default); `Settings::save(&self, path: &Path) -> std::io::Result<()>`.

- [ ] **Step 1: Escrever o teste que falha**

```rust
// src/config.rs (no fim do arquivo)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_prd() {
        let s = Settings::default();
        assert_eq!(s.hotkey, HotkeyChoice::PrtScn);
        assert!(!s.start_with_windows);
        assert_eq!(s.stroke_width, 3);
        assert_eq!(s.text_size, 18);
        assert_eq!(s.default_color, Color::Red);
        assert!(!s.text_background);
    }

    #[test]
    fn invalid_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("shot_cfg_invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "{ isto não é json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
    }

    #[test]
    fn round_trip_save_load() {
        let dir = std::env::temp_dir().join("shot_cfg_rt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        let s = Settings { stroke_width: 7, default_color: Color::Blue, ..Settings::default() };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test config::`
Expected: FAIL — `Settings` não definido.

- [ ] **Step 3: Implementação mínima**

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyChoice { PrtScn, ShiftPrtScn }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color { Yellow, Blue, Green, Red, Black, White }

impl Color {
    pub const ALL: [Color; 6] = [Color::Yellow, Color::Blue, Color::Green, Color::Red, Color::Black, Color::White];
    /// Cor em formato 0x00BBGGRR (COLORREF do Win32).
    pub fn colorref(self) -> u32 {
        match self {
            Color::Yellow => 0x0000FFFF,
            Color::Blue => 0x00FF0000 | 0x00000000, // azul puro: B=255
            Color::Green => 0x0000C000,
            Color::Red => 0x000000FF,
            Color::Black => 0x00000000,
            Color::White => 0x00FFFFFF,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey: HotkeyChoice,
    pub start_with_windows: bool,
    pub stroke_width: u32,
    pub text_size: u32,
    pub default_color: Color,
    pub text_background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: HotkeyChoice::PrtScn,
            start_with_windows: false,
            stroke_width: 3,
            text_size: 18,
            default_color: Color::Red,
            text_background: false,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let s = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, s)
    }
}
```

Nota: o bloco `Color::Blue` acima deve ser `0x00FF0000` (azul puro em COLORREF). Corrija ao escrever o arquivo.

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test config::`
Expected: PASS (3 testes).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add settings model with JSON load/save"
```

---

### Task 2: Geometria (geom.rs)

**Files:**
- Create: `src/geom.rs`
- Modify: `src/main.rs` (`mod geom;`)

**Interfaces:**
- Produces:
  - `pub struct Point { pub x: i32, pub y: i32 }`, `pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }` (ambos `Copy`, `PartialEq`, `Debug`).
  - `Rect::from_points(a: Point, b: Point) -> Rect` (normaliza; largura/altura ≥ 0).
  - `Rect::contains(&self, p: Point) -> bool`.
  - `Rect::clamp_to(&self, bounds: Rect) -> Rect`.
  - `pub enum Handle { NW, N, NE, E, SE, S, SW, W }`; `Rect::handles(&self) -> [(Handle, Point); 8]`; `Rect::hit_handle(&self, p: Point, tol: i32) -> Option<Handle>`; `Rect::resize_by_handle(&self, h: Handle, p: Point) -> Rect`.
  - `pub fn constrain_45(from: Point, to: Point) -> Point` (linha a 45°: alinha ao eixo ou diagonal mais próxima).
  - `pub fn square_or_circle(from: Point, to: Point) -> Rect` (lado = max(|dx|,|dy|), mantém direção).
  - `pub fn dimension_label(r: Rect) -> String` → `"206x143"`.
  - `pub struct BarLayout { pub vertical: Rect, pub horizontal: Rect }` e `pub fn layout_bars(sel: Rect, screen: Rect, bar_w: i32, bar_h: i32, v_items: usize, h_items: usize, item: i32) -> BarLayout` — regra: barra vertical encostada no lado direito, alinhada embaixo; horizontal encostada embaixo, alinhada à direita; se não couber fora da seleção/tela, passa para dentro.

- [ ] **Step 1: Testes que falham**

```rust
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
        let l = layout_bars(sel, screen, 40, 40, 11, 3, 40);
        assert!(l.vertical.x + l.vertical.w <= 600, "barra deve ficar dentro da seleção");
        assert!(l.vertical.x >= 600 - 40 - 8);
    }

    #[test]
    fn bars_outside_when_room() {
        let screen = Rect { x: 0, y: 0, w: 1000, h: 800 };
        let sel = Rect { x: 100, y: 100, w: 200, h: 200 };
        let l = layout_bars(sel, screen, 40, 40, 11, 3, 40);
        assert_eq!(l.vertical.x, 300 + 8);
        assert_eq!(l.horizontal.y, 300 + 8);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test geom::`
Expected: FAIL — itens não definidos.

- [ ] **Step 3: Implementação**

Implementar as funções listadas em Interfaces. Regras fixas:
- `GAP = 8` px entre seleção e barra.
- Barra vertical: `x = sel.right + GAP`; altura `v_items * item`; `y = sel.bottom - altura`. Se `x + bar_w > screen.right`, usar `x = sel.right - GAP - bar_w` (dentro).
- Barra horizontal: `y = sel.bottom + GAP`; largura `h_items * item`; `x = sel.right - largura`. Se `y + bar_h > screen.bottom`, usar `y = sel.bottom - GAP - bar_h` (dentro).
- `resize_by_handle`: move só as bordas afetadas; garantir `w,h ≥ 1`.

- [ ] **Step 4: Passar**

Run: `cargo test geom::` → PASS.

- [ ] **Step 5: Commit**

```bash
git add src/geom.rs src/main.rs
git commit -m "feat: add selection geometry, handles and toolbar layout"
```

---

### Task 3: Modelo de anotações (model.rs)

**Files:**
- Create: `src/model.rs`
- Modify: `src/main.rs` (`mod model;`)

**Interfaces:**
- Consumes: `geom::{Point, Rect}`, `config::Color`.
- Produces:
  - `pub enum Annotation { Pen { points: Vec<Point>, color: Color, width: u32 }, Line { a: Point, b: Point, color, width }, Arrow { a, b, color, width }, Rect { r: Rect, color, width }, Ellipse { r: Rect, color, width }, Marker { r: Rect, color }, Text { at: Point, text: String, color, size: u32, background: bool }, Pixelate { r: Rect }, Number { center: Point, n: u32, color } }`
  - `pub struct Document { items: Vec<Annotation>, selected: Option<usize>, undo: Vec<Vec<Annotation>>, redo: Vec<Vec<Annotation>> }`
  - `Document::new()`, `items(&self) -> &[Annotation]`, `push(&mut self, a: Annotation)` (grava snapshot para undo, limpa redo), `undo(&mut self) -> bool`, `redo(&mut self) -> bool`, `select_at(&mut self, p: Point) -> Option<usize>`, `delete_selected(&mut self) -> bool` (renumera os `Number` restantes em ordem de criação), `move_selected(&mut self, dx, dy)` (com snapshot na primeira chamada de um gesto: usar `begin_gesture()` / `end_gesture()`), `next_number(&self) -> u32`.
  - `pub fn bounds_of(a: &Annotation) -> Rect` (para hit-test e para pixelização).

- [ ] **Step 1: Testes que falham**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Color;
    use crate::geom::Point;

    fn num(n: u32, x: i32) -> Annotation {
        Annotation::Number { center: Point { x, y: 0 }, n, color: Color::Red }
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut d = Document::new();
        d.push(num(1, 0));
        d.push(num(2, 10));
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
        let ns: Vec<u32> = d.items().iter().filter_map(|a| match a {
            Annotation::Number { n, .. } => Some(*n), _ => None }).collect();
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
        assert!(d.undo());
        assert_eq!(d.items().len(), 1);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test model::` → FAIL.

- [ ] **Step 3: Implementação**

Regras:
- `bounds_of` para `Pen` = retângulo que envolve os pontos; `Line`/`Arrow` = `from_points(a,b)`; `Number` = quadrado de 2·raio centrado; `Text` = `Rect{at.x, at.y, 200, size*2}` aproximado (a medida real vem da pintura; ver Task 9).
- `push` empilha `items.clone()` em `undo`; `redo.clear()`.
- `undo`: move snapshot atual para `redo`, restaura o último de `undo`.
- `delete_selected`: remove o item; se for `Number`, renumera os demais `Number` por ordem de aparição no vetor (1..k) e grava o snapshot antes.
- `next_number` = maior `n` existente + 1 (ou 1).
- `begin_gesture` grava snapshot se ainda não houver gesto; `end_gesture` limpa o flag. `move_selected` não grava snapshot.

- [ ] **Step 4: Passar** — `cargo test model::` → PASS.

- [ ] **Step 5: Commit**

```bash
git add src/model.rs src/main.rs
git commit -m "feat: add annotation document with undo/redo and renumbering"
```

---

### Task 4: Pixelização (pixelate.rs)

**Files:**
- Create: `src/pixelate.rs`
- Modify: `src/main.rs` (`mod pixelate;`)

**Interfaces:**
- Produces: `pub fn pixelate_region(buf: &mut [u8], stride: usize, img_w: i32, img_h: i32, r: Rect, block: u32)` — `buf` é BGRA (4 bytes/px), `stride` em bytes. Recorta `r` nos limites da imagem; cada bloco `block×block` recebe a média dos seus pixels (sobrescrita irreversível).

- [ ] **Step 1: Teste que falha**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;

    #[test]
    fn block_becomes_average_and_original_gone() {
        // 4x1 BGRA: preto, branco, preto, branco
        let mut buf = vec![
            0,0,0,255,  255,255,255,255,  0,0,0,255,  255,255,255,255,
        ];
        pixelate_region(&mut buf, 16, 4, 1, Rect { x: 0, y: 0, w: 4, h: 1 }, 4);
        assert!(buf.chunks(4).all(|p| p[0] == 127 || p[0] == 128));
    }

    #[test]
    fn region_clipped_to_image() {
        let mut buf = vec![9u8; 4 * 2 * 2];
        pixelate_region(&mut buf, 8, 2, 2, Rect { x: 1, y: 1, w: 50, h: 50 }, 8);
        assert_eq!(buf[0], 9); // pixel fora da região intacto
    }
}
```

- [ ] **Step 2: FAIL** — `cargo test pixelate::`.
- [ ] **Step 3: Implementar** — para cada bloco dentro do recorte: somar canais B,G,R,A, dividir pelo número de pixels, escrever de volta. Usar `u32` na soma.
- [ ] **Step 4: PASS** — `cargo test pixelate::`.
- [ ] **Step 5: Commit** — `git add src/pixelate.rs src/main.rs && git commit -m "feat: add irreversible block pixelation"`

---

### Task 5: Captura de tela (capture.rs)

**Files:**
- Create: `src/capture.rs`
- Modify: `src/main.rs` (`mod capture;`)

**Interfaces:**
- Produces: `pub struct Capture { pub origin: Point, pub width: i32, pub height: i32, pub pixels: Vec<u8> }` (BGRA top-down, `stride = width*4`). `pub fn capture_virtual_screen() -> windows::core::Result<Capture>`.
- Consumes: `geom::Point`.

Implementação: `GetSystemMetrics(SM_XVIRTUALSCREEN/YVIRTUALSCREEN/CXVIRTUALSCREEN/CYVIRTUALSCREEN)`; `GetDC(None)` → `CreateCompatibleDC` + `CreateDIBSection` (BITMAPINFOHEADER com `biHeight = -h`, 32 bpp, `BI_RGB`) → `BitBlt(SRCCOPY)` da tela para o DIB → copia os bytes para `Vec<u8>` → `DeleteObject`/`DeleteDC`/`ReleaseDC`. Chamar `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` em `main` antes desta função (ver Task 7).

- [ ] **Step 1: Teste de fumaça (manual com `--debug`)** — não há teste automatizado de tela. Critério: `cargo run -- --capture-test` salva `%TEMP%\cap_test.png`? **Não**: a regra de segurança diz sem arquivos temporários. Em vez disso, o teste é: `capture_virtual_screen()` retorna `width == GetSystemMetrics(SM_CXVIRTUALSCREEN)` e `pixels.len() == w*h*4`. Escrever como `#[test] #[ignore]` (precisa de sessão de desktop):

```rust
#[test]
#[ignore = "requer sessão de desktop"]
fn capture_has_expected_size() {
    let c = capture_virtual_screen().unwrap();
    assert!(c.width > 0 && c.height > 0);
    assert_eq!(c.pixels.len(), (c.width * c.height * 4) as usize);
}
```

- [ ] **Step 2: Implementar** conforme descrição.
- [ ] **Step 3: Rodar** `cargo test capture -- --ignored` → PASS.
- [ ] **Step 4: Commit** — `git commit -m "feat: capture full virtual screen to BGRA buffer"`

---

### Task 6: Bandeja e atalho global (tray.rs, hotkey.rs, main.rs)

**Files:**
- Create: `src/tray.rs`, `src/hotkey.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces:
  - `pub const WM_APP_TRAY: u32 = WM_APP + 1;` `pub const ID_HOTKEY: i32 = 1;` `pub const ID_MENU_EXIT: u16 = 1001;` `pub const ID_MENU_SETTINGS: u16 = 1002;`
  - `tray::add(hwnd: HWND) -> Result<()>`, `tray::remove(hwnd: HWND)`, `tray::show_menu(hwnd: HWND)`.
  - `hotkey::register(hwnd: HWND, choice: HotkeyChoice) -> Result<HotkeyChoice, String>`: tenta o escolhido; se falhar com PrtScn, retorna `Err` com a mensagem para o usuário ("A tecla PrtScn está em uso pela Ferramenta de Captura. Desative em Configurações › Acessibilidade › Teclado ou use Shift+PrtScn.") e tenta Shift+PrtScn automaticamente.

Notas Win32:
- Ícone: `LoadIconW(None, IDI_APPLICATION)` no começo (sem recurso). Tooltip "Screenshotter".
- `RegisterHotKey(hwnd, ID_HOTKEY, MOD_NOREPEAT | mods, VK_SNAPSHOT.0 as u32)`.
- Janela de mensagens: `CreateWindowExW` com classe própria `ShotMainWnd`, `WS_POPUP`, invisível. `main` cria, chama `tray::add` e `hotkey::register`, depois `GetMessageW`/`DispatchMessageW`.
- `WM_HOTKEY` (wParam == ID_HOTKEY) → `overlay::start_capture()`.
- Clique direito no ícone (`WM_APP_TRAY`, `lparam` = `WM_RBUTTONUP`) → `show_menu`, com `SetForegroundWindow` antes de `TrackPopupMenu` (senão o menu não fecha sozinho).

- [ ] **Step 1:** Não há teste unitário viável; critério verificável é manual: rodar o `.exe`, ver o ícone, apertar PrtScn (com a Ferramenta de Captura desligada) e ver o log `hotkey ok`.
- [ ] **Step 2:** Implementar `tray.rs` e `hotkey.rs`.
- [ ] **Step 3:** `cargo build` → compila sem warnings de `unused` nos módulos já usados.
- [ ] **Step 4:** Commit — `git commit -m "feat: tray icon and global PrtScn hotkey with fallback"`

---

### Task 7: Janela de sobreposição e pintura (overlay.rs)

**Files:**
- Create: `src/overlay.rs`, `src/editor.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `capture::Capture`, `geom::*`, `model::Document`, `config::Settings`.
- Produces:
  - `overlay::create(hwnd_owner) -> Result<HWND>`: cria `WS_POPUP`, `WS_EX_TOPMOST | WS_EX_TOOLWINDOW`, classe `ShotOverlayWnd`, **oculta**. Tamanho = área virtual; reposicionada no `start_capture`.
  - `overlay::start_capture()`: `capture_virtual_screen()` → `editor::Editor::new(capture, settings)` → `SetWindowPos(HWND_TOPMOST, origin, size, SWP_SHOWWINDOW)` → `SetForegroundWindow` → `SetCapture`.
  - `editor::Editor { capture, settings, doc, sel: Option<Rect>, tool, color, stroke, text_size, state: Interaction }`, com `Editor::new`.
  - Pintura: `paint(hdc, editor)`. Buffer duplo: `CreateCompatibleBitmap` com o tamanho da área virtual, desenha tudo no backbuffer e faz `BitBlt` na tela.
  - Fundo: o bitmap capturado é desenhado escurecido (alpha blend 50% com preto via `AlphaBlend` de um retângulo preto, ou `SetDIBitsToDevice` + `AlphaBlend` com `BLENDFUNCTION` SourceConstantAlpha=128); a área selecionada é desenhada sem escurecer por cima.

- [ ] **Step 1:** Teste de máquina de estados em `editor.rs` (puro, sem Win32):

```rust
#[test]
fn drag_creates_selection_and_release_fixes_it() {
    let mut e = Editor::test_new(1920, 1080);
    e.on_mouse_down(Point { x: 10, y: 10 });
    e.on_mouse_move(Point { x: 110, y: 60 });
    assert_eq!(e.sel, Some(Rect { x: 10, y: 10, w: 100, h: 50 }));
    e.on_mouse_up();
    assert!(matches!(e.state, Interaction::Idle));
    assert!(e.sel.is_some());
}
```

- [ ] **Step 2:** FAIL, implementar `Editor` com `Interaction { Idle, Selecting{anchor}, ResizingSel{handle}, Drawing{..}, Moving{last}, }`.
- [ ] **Step 3:** Implementar `overlay.rs` e `paint`. Critério manual: apertar PrtScn → tela escurece; sem seleção, nada mais.
- [ ] **Step 4:** `cargo test editor::` → PASS; `cargo build` ok.
- [ ] **Step 5:** Commit — `git commit -m "feat: topmost overlay with darkened capture and double-buffered paint"`

---

### Task 8: Seleção com medidas e alças (editor.rs + overlay.rs)

**Files:**
- Modify: `src/editor.rs`, `src/overlay.rs`

**Interfaces:**
- Consumes: `geom::{Rect, Handle, dimension_label}`.
- Produces: `Editor::on_mouse_down/move/up(p)`; pintura do contorno tracejado (`PS_DASH`, 1 px, branco com contorno preto) e 8 alças quadradas de 6 px; etiqueta escura com `dimension_label` acima do canto superior esquerdo (dentro da tela: se `y - 22 < 0`, desenhar dentro da seleção).

- [ ] **Step 1: Testes** — `resize_handle_drag_changes_rect`, `handle_drag_keeps_min_size_1`.
- [ ] **Step 2: FAIL** → **Step 3: Implementar** → **Step 4: PASS**.
- [ ] **Step 5: Commit** — `git commit -m "feat: selection drag with dashed outline, handles and size label"`

---

### Task 9: Barras de ícones (toolbar.rs)

**Files:**
- Create: `src/toolbar.rs`
- Modify: `src/overlay.rs`, `src/editor.rs`

**Interfaces:**
- Produces:
  - `pub enum Button { Pen, Line, Arrow, Rect, Ellipse, Marker, Text, Pixelate, Number, ColorSwatch, Undo, Copy, Save, Close }`
  - `toolbar::vertical_buttons() -> [Button; 11]` (Pen … ColorSwatch, Undo) — ordem do PRD F7.
  - `toolbar::horizontal_buttons() -> [Button; 3]` (Copy, Save, Close).
  - `toolbar::hit(layout: &BarLayout, p: Point) -> Option<Button>`.
  - `toolbar::paint_bar(hdc, rect, buttons, active: Option<Button>, color: Color)`.
  - `toolbar::palette_rect(swatch_rect) -> Rect` e `palette_hit(p) -> Option<Color>` (6 quadradinhos ao lado do swatch).

Ícones: desenhar com GDI primitivas simples (caneta = curva; linha = `MoveToEx/LineTo`; seta = linha + ponta; retângulo/elipse = `Rectangle`/`Ellipse` com `NULL_BRUSH`; marca-texto = retângulo amarelo translúcido; texto = "T"; pixelizar = grade 3×3; numerar = círculo com "1"; desfazer = seta curva; copiar = dois retângulos; salvar = disquete simplificado; fechar = "X"). Fundo da barra: branco com borda cinza; botão ativo com fundo azul claro.

- [ ] **Step 1: Testes** — `vertical_bar_fits_inside_screen` (usar `geom::layout_bars`), `hit_returns_correct_button`, `palette_hit_maps_six_colors`.
- [ ] **Step 2–4:** FAIL → implementar → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: vertical and horizontal icon bars anchored to selection"`

---

### Task 10: Ferramentas, cor, espessura e atalhos (editor.rs)

**Files:**
- Modify: `src/editor.rs`, `src/overlay.rs`

**Interfaces:**
- Produces:
  - `Editor::set_tool(Tool)`, `Editor::set_color(Color)` (também fecha a paleta), `Editor::on_wheel(delta: i16, ctrl: bool)` (espessura das formas, ou tamanho do texto se a ferramenta é Texto ou se a caixa de texto está aberta).
  - Ferramentas com arrastar: Pen (acumula pontos), Line/Arrow (shift → `constrain_45`), Rect/Ellipse (shift → `square_or_circle`), Marker (retângulo semitransparente: `AlphaBlend` com 40%), Pixelate (retângulo, `pixelate::pixelate_region` aplicado **na exportação**; na tela aparece como retângulo hachurado).
  - Clique simples: Number → `Annotation::Number { n: doc.next_number() }`; Text → abre caixa de texto no ponto.
  - Teclado: `P L A R E M T B N` troca ferramenta; `1..6` troca cor; `Ctrl+Z`, `Ctrl+Y`, `Del`; ignorar letras/números quando a caixa de texto está aberta.
  - Shift lido com `GetKeyState(VK_SHIFT)` no `WM_MOUSEMOVE`.

- [ ] **Step 1: Testes puros** (em `editor.rs`):

```rust
#[test]
fn shift_line_is_45_degrees() {
    let mut e = Editor::test_new(800, 600);
    e.set_tool(Tool::Line);
    e.on_mouse_down(Point { x: 0, y: 0 });
    e.on_mouse_move_with_shift(Point { x: 100, y: 30 }, true);
    e.on_mouse_up();
    assert!(matches!(e.doc.items()[0], Annotation::Line { b: Point { x: 30, y: 30 }, .. }));
}

#[test]
fn number_tool_increments() {
    let mut e = Editor::test_new(800, 600);
    e.set_tool(Tool::Number);
    e.on_click(Point { x: 10, y: 10 });
    e.on_click(Point { x: 50, y: 50 });
    let ns: Vec<u32> = e.doc.items().iter().filter_map(|a| match a { Annotation::Number{n,..} => Some(*n), _ => None }).collect();
    assert_eq!(ns, vec![1, 2]);
}

#[test]
fn wheel_changes_stroke_within_bounds() {
    let mut e = Editor::test_new(800, 600);
    e.stroke = 1;
    e.on_wheel(-120, false);
    assert_eq!(e.stroke, 1); // não desce abaixo de 1
    e.on_wheel(120, false);
    assert_eq!(e.stroke, 2);
}
```

Onde `Tool` é `enum Tool { Pen, Line, Arrow, Rect, Ellipse, Marker, Text, Pixelate, Number }` (definido em `editor.rs`); `Editor::test_new(w,h)` cria editor com config padrão e captura vazia.

- [ ] **Step 2–4:** FAIL → implementar → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: drawing tools, color/stroke controls and editor shortcuts"`

---

### Task 11: Texto inline (editor.rs + overlay.rs)

**Files:** Modify `src/editor.rs`, `src/overlay.rs`.

**Interfaces:**
- Produces: `Editor::text_edit: Option<TextEdit { at: Point, buf: String }>`; `on_char(c: char)` (Enter → `\n`; Backspace remove último char); `commit_text()` (cria `Annotation::Text` se não vazio); `on_click` fora da caixa → `commit_text()`; Esc → `commit_text()` e não cancela a captura.
- Pintura: `CreateFontW` com `-text_size` de altura, `DrawTextW` com fundo `background` opcional (`SetBkMode(OPAQUE)` + `SetBkColor(branco)`), cor = `color.colorref()`.
- Sincronizar `bounds_of` do `Text` com a medida real (`GetTextExtentPoint32W`) ao criar.

- [ ] **Step 1: Testes**

```rust
#[test]
fn text_commits_on_click_outside_and_esc_keeps_capture() {
    let mut e = Editor::test_new(800, 600);
    e.set_tool(Tool::Text);
    e.on_click(Point { x: 20, y: 20 });
    e.on_char('o'); e.on_char('i');
    e.on_click(Point { x: 300, y: 300 });
    assert!(matches!(&e.doc.items()[0], Annotation::Text { text, .. } if text == "oi"));
}

#[test]
fn letter_shortcuts_disabled_while_typing() {
    let mut e = Editor::test_new(800, 600);
    e.set_tool(Tool::Text);
    e.on_click(Point { x: 20, y: 20 });
    e.on_key_letter('p');
    assert_eq!(e.tool, Tool::Text);
}
```

- [ ] **Step 2–4:** FAIL → implementar → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: inline text tool with background option"`

---

### Task 12: Exportação, copiar e salvar (export.rs)

**Files:** Create `src/export.rs`; modify `src/overlay.rs`, `src/main.rs`.

**Interfaces:**
- Produces:
  - `export::compose(cap: &Capture, sel: Rect, doc: &Document) -> Vec<u8>`: BGRA `sel.w × sel.h`. (1) copia a área de `cap.pixels`; (2) aplica `pixelate_region` em cada `Annotation::Pixelate` (coordenadas relativas à seleção); (3) desenha as formas vetoriais por cima com GDI num DIB do tamanho da seleção (`compose` chama `draw::paint_annotations`, função pura de pintura reutilizada pela tela — ver nota).
  - `export::to_png(bgra: &[u8], w: u32, h: u32, path: &Path) -> std::io::Result<()>` usando crate `png` (converter BGRA→RGBA; `ColorType::Rgba`).
  - `export::copy_to_clipboard(hwnd, bgra, w, h) -> Result<()>`: `OpenClipboard`, `EmptyClipboard`, `GlobalAlloc(GMEM_MOVEABLE)` com `BITMAPINFOHEADER` (biHeight negativo, 32 bpp) + pixels, `SetClipboardData(CF_DIB)`, `CloseClipboard`.
  - `export::save_dialog(hwnd, suggested: &str) -> Option<PathBuf>`: `GetSaveFileNameW` com filtro `*.png`, `OFN_OVERWRITEPROMPT`, nome inicial `Captura_YYYY-MM-DD_HH-MM-SS.png`. **Sem pasta padrão** (decisão do PRD).
  - Teste: `compose` com um Pixelate sobre fundo de 2 cores produz pixels médios (teste puro com `Capture` sintético e sem desenho vetorial).

- Nota de implementação: a pintura de anotações é uma função `draw::paint_annotations(hdc: HDC, origin: Point, items: &[Annotation], sel_offset: Point)` usada tanto pela tela (com `origin` da área virtual) quanto pela exportação (com `origin` da seleção). Isso garante que o que se vê é o que se copia.

- [ ] **Step 1: Teste** — `pixelate_is_applied_on_compose`.
- [ ] **Step 2–4:** FAIL → implementar → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: export with irreversible pixelation, clipboard CF_DIB and PNG save dialog"`

---

### Task 13: Ligação final e medição de latência (overlay.rs, log.rs)

**Files:** Create `src/log.rs`; modify `src/overlay.rs`, `src/main.rs`.

**Interfaces:**
- `log::latency(label: &str, start: Instant)` — escreve em `stderr` só se a flag `--debug` estiver ativa (`static DEBUG: AtomicBool`).
- Fluxo completo: `WM_HOTKEY` → `start_capture` (registra `Instant` no começo) → após o primeiro `WM_PAINT` com a tela congelada, `latency("hotkey->frame", t0)`. Ctrl+C → `latency("copy", t0)`.
- Teclado da sobreposição: Ctrl+C copia e fecha; Ctrl+S abre o diálogo; Esc cancela e oculta (`ShowWindow(SW_HIDE)`) sem gravar nada; Ctrl+Z/Y; Del.
- Ao fechar: liberar `Capture` (`editor` descartado) — sem rastro em memória além do necessário.

- [ ] **Step 1: Teste de integração manual** (checklist — ver Task 14).
- [ ] **Step 2:** Ligar eventos, `cargo build --release`.
- [ ] **Step 3:** Commit — `git commit -m "feat: wire overlay events, copy/save/cancel and latency logging"`

---

### Task 14: Configurações, iniciar com Windows e verificação final

**Files:** Create `src/startup.rs`, `src/settings_window.rs`; modify `src/tray.rs`, `src/main.rs`.

**Interfaces:**
- `startup::set_enabled(on: bool) -> Result<()>`: grava/remove `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` valor `Screenshotter` = caminho do `.exe` entre aspas. Chamado ao salvar configurações.
- `settings_window::open(hwnd)`: janela com controles padrão (`BUTTON` checkbox para iniciar com Windows e para fundo do texto; `EDIT` numérico para espessura e tamanho; `COMBOBOX` para cor padrão e atalho; botões Salvar/Cancelar). Salvar → `Settings::save` em `settings.json` ao lado do `.exe` (`std::env::current_exe()`), e aplica `startup::set_enabled`.
- Se a janela não ficar pronta, o item de menu "Configurações" abre `settings.json` no Bloco de Notas (`ShellExecuteW` "open" no arquivo) como fallback. Registrar essa decisão no commit.

**Verificação final (checklist de aceite do PRD, marcar um por um com evidência):**
- [ ] Mediana atalho → seleção ≤ 100 ms em 20 execuções (log `--debug`).
- [ ] Etiqueta de medidas bate com o PNG salvo (abrir no Paint: largura × altura).
- [ ] Barras nunca saem da tela com seleção encostada na borda.
- [ ] Captura correta em 2 monitores com escalas diferentes (se houver hardware; senão, registrar como não verificado).
- [ ] 6 cores, espessura e tamanho de texto funcionam em todas as ferramentas.
- [ ] Padrões de configuração aplicados a cada captura nova.
- [ ] Todo objeto move e apaga.
- [ ] Área pixelizada não recuperável no PNG (inspecionar com editor hexadecimal ou comparar pixels).
- [ ] Ctrl+C cola no Paint, Word e Teams/Slack.
- [ ] Zero conexões: `Get-NetTCPConnection -OwningProcess` vazio durante uso; `Cargo.toml` sem crates de rede (`Select-String -Path Cargo.toml -Pattern 'reqwest|hyper|ureq|tokio|socket'` vazio).
- [ ] RAM na bandeja ≤ 30 MB; `.exe` ≤ 10 MB (`cargo build --release` + `Get-Item target\release\screenshotter.exe`).

- [ ] **Step 1–3:** Implementar, `cargo build --release`.
- [ ] **Step 4:** Executar o checklist acima e registrar resultados no `docs/superpowers/plans/2026-11-04-screenshotter-v1-verificacao.md`.
- [ ] **Step 5: Commit** — `git commit -m "feat: settings window, start-with-Windows and acceptance checks"`

---

## Self-Review (feito ao escrever o plano)

1. **Cobertura do PRD:** F1 (T6, T14) · F2 (T6) · F3/F4/F5 (T2, T8) · F6 (T5, T7 DPI) · F7 (T2, T9) · F8 (T10) · F9 (T10) · F10 (T10) · F11 (T9, T10) · F12 (T10) · F13/F14 (T11) · F15 (T3, T10) · F16 (T4, T12) · F17 (T3, T10) · F18 (T3, T10) · F19 (T12) · F20 (T12) · F21 (T7, T13) · F22 (T1, T14) · Metas (T13) · Critérios de aceite (T14). Sem lacuna.
2. **Placeholders:** blocos "Implementar conforme descrição" referem a regras explícitas nas Interfaces; os passos de teste têm código real.
3. **Consistência de tipos:** `Point`/`Rect` (geom) usados em todas as tasks; `Color::colorref` (T1) usado em T11/T12; `Tool` definido em T10 e referenciado em T11; `Capture` (T5) consumido em T7 e T12; `Document` (T3) consumido em T10 e T12.

## Execution Handoff

Plano pronto. Execução inline com `superpowers:executing-plans`, em lotes com checkpoints.
