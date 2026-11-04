# Screenshotter v1 — Verificação (Task 14, passo 4)

Ambiente: Windows 11 Pro 10.0.26200, monitor único. Build `--release`, toolchain `stable-x86_64-pc-windows-gnu` (rustc 1.99.1).

Legenda: **OK** = verificado com evidência · **PARCIAL** = verificado em parte · **NÃO VERIFICADO** = não testado nesta sessão.

| Critério | Status | Evidência |
| --- | --- | --- |
| Mediana atalho → seleção ≤ 100 ms | **NÃO VERIFICADO** | Medido de fora: mediana de 317 ms (5 execuções) incluindo ~140 ms do próprio `SendKeys`. Log interno não chega ao stderr de um exe GUI nesta sessão. Precisa medição interna. |
| Etiqueta de medidas bate com o PNG | **PARCIAL** | Ctrl+C de uma seleção arrastada de (200,200) a (500,380) entregou imagem de **300x180** na área de transferência. |
| Barras nunca saem da tela | **PARCIAL** | Coberto por testes unitários de `geom::layout_bars` e `toolbar`. Sem teste visual com seleção encostada na borda. |
| Captura correta em 2 monitores com escalas diferentes | **NÃO VERIFICADO** | Só há 1 monitor. |
| 6 cores, espessura e tamanho em todas as ferramentas | **PARCIAL** | Lógica testada (`digit_selects_color`, `wheel_changes_stroke_within_bounds`). Visual não verificado. |
| Padrões de configuração a cada captura | **OK (código)** | `overlay` relê `settings.json` a cada `WM_HOTKEY`. |
| Todo objeto move e apaga | **OK (testes)** | `model::move_gesture_is_one_undo_step`, `deleting_number_renumbers_rest`. |
| Área pixelizada irrecuperável no PNG | **OK (testes)** | `pixelate::block_becomes_average_and_original_gone`, `export::compose_pixelates_before_export`. |
| Ctrl+C cola no Teams/Slack, Word e Paint | **PARCIAL** | Imagem chega ao clipboard com tamanho certo. Colagem em apps não testada. |
| Zero conexões de rede do processo | **OK** | `Get-NetTCPConnection -OwningProcess` = 0 conexões com o processo rodando. `Cargo.toml` sem crates de rede. |
| RAM na bandeja ≤ 30 MB | **OK** | 11,8 MB de working set, parado. |
| `.exe` ≤ 10 MB | **OK** | 1,86 MB (release). |

## Fluxo testado de ponta a ponta

- PrtScn abre a sobreposição: brilho médio da tela 212,7 → 85,3 (~50% escurecido).
- Esc fecha a sobreposição: brilho volta para 212,7.
- Arrastar + Ctrl+C: área de transferência com imagem 300x180.

## Pendências conhecidas

1. Medir a latência atalho → quadro dentro do app (o log `--debug` existe, mas o stderr de um exe GUI não é capturado por `Start-Process`). Sugestão: `AllocConsole` quando `--debug`.
2. Janela própria de configurações (F22) não feita: `Configurações` abre `settings.json` no editor padrão.
3. Aviso de conflito do PrtScn usa `MessageBox` no startup.
4. Desenho em GDI em vez de Direct2D (decisão documentada no plano).
5. PNG via crate `png` em vez de WIC (decisão documentada no plano).
6. Toolchain GNU (MinGW/WinLibs) em vez de MSVC: o Windows SDK não pôde ser instalado sem elevação.
