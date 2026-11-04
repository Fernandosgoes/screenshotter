# PRD — Screenshotter local (Windows)

Nov 4, 2026 · @Fernando Goes

## Resumo e problema

Um capturador de tela para Windows, de uso pessoal, que faz o que o Lightshot faz de melhor (selecionar, anotar, copiar, salvar) sem nunca enviar nada para a internet.

**Por que não usar o Lightshot:**

- **Nuvem insegura.** Os links do prnt.sc usam um ID curto e previsível, e há crawlers que coletam uploads públicos ([CodeGenes](https://www.codegenes.net/blog/lightshot-download/)). Fontes são majoritariamente blogs de concorrentes; o problema, porém, é amplamente relatado.
- **Instalador com extras.** O AlternativeTo marca o Lightshot como bundleware, oferecendo produtos da Yandex ([AlternativeTo](https://alternativeto.net/software/lightshot/about)).
- **Faltam ferramentas.** Sem desfoque/pixelização, sem passos numerados, e a edição é estilo Paint, sem objetos editáveis ([AnyMP4](https://www.anymp4.com/recorder/lightshot-review.html), [AlternativeTo](https://alternativeto.net/software/lightshot/about)).

**Usuário:** o próprio autor (Windows). Secundário: colegas de trabalho que recebem o `.exe` diretamente.

## Objetivos, não-objetivos e princípios

**Objetivos da v1**

1. Apertar o atalho e a seleção aparecer imediatamente, sem atraso perceptível.
2. Anotar com as ferramentas do Lightshot + desfoque/pixelização + passos numerados.
3. Copiar (Ctrl+C) ou salvar (Ctrl+S) e voltar ao trabalho.

**Fora do escopo (decisão do dono, não rediscutir na v1)**

- Qualquer upload, nuvem, link de compartilhamento ou servidor.
- Busca de imagens parecidas, editor online, contas, login.
- Telemetria, analytics, atualização automática pela internet.
- Gravação de vídeo/GIF, captura com rolagem, OCR, Mac/Linux.

**Princípios**

- **Zero rede.** O programa não abre nenhuma conexão. Verificável: o código não importa bibliotecas de rede, e o firewall pode bloquear o `.exe` sem quebrar nada.
- **Um arquivo só.** Portátil, sem instalador, sem runtime para instalar.
- **Leve antes de completo.** Toda feature nova precisa caber nas metas de desempenho; se não couber, não entra.

## Requisitos funcionais da v1

A v1 replica o fluxo do Lightshot (atalho → seleção → anotação → copiar/salvar) e acrescenta três coisas: objetos editáveis, desfoque e passos numerados. P0 = sem isso não lança; P1 = entra se couber nas metas.

| ID | Requisito | Prioridade |
| --- | --- | --- |
| F1 | Roda na bandeja do sistema; inicia com o Windows (opcional, desligado por padrão) | P0 |
| F2 | Atalho global **PrtScn** (padrão) congela a tela e abre a seleção. Se o Windows não liberar o PrtScn, o app avisa e oferece **Shift+PrtScn**. Configurável | P0 |
| F3 | **Seleção de área:** arrastar desenha um contorno tracejado em volta da área; o resto da tela fica escurecido | P0 |
| F4 | **Medidas da área:** etiqueta escura com largura × altura em pixels (ex.: 206x143) logo acima do canto superior esquerdo da seleção, atualizando enquanto arrasta. Contorno tracejado com 8 alças (cantos e meio das bordas) | P0 |
| F5 | Seleção ajustável por alças nos cantos e bordas depois de feita | P0 |
| F6 | Funciona em múltiplos monitores e com escala de DPI diferente por monitor | P0 |
| F7 | **Ícones na borda, layout do Lightshot:** (a) **barra vertical** encostada no lado direito da seleção, alinhada embaixo: Caneta, Linha, Seta, Retângulo, Elipse, Marca-texto, Texto, Pixelizar, Numerar, quadradinho da cor ativa, Desfazer. (b) **barra horizontal** encostada embaixo da seleção, alinhada à direita: Copiar, Salvar, Fechar. Os ícones de nuvem, compartilhar, Google e imprimir do Lightshot **não existem** aqui. Se a seleção encostar na borda da tela, a barra passa para o lado de dentro da seleção em vez de sumir | P0 |
| F8 | **Ferramentas de forma:** linha reta, seta, retângulo, círculo/elipse. **Retângulo e elipse só com contorno, sem preenchimento** (como no Lightshot). Clicar e arrastar define início e fim; Shift trava a linha em 45° e deixa retângulo e elipse perfeitos (quadrado e círculo) | P0 |
| F9 | **Caneta livre** para rabiscar | P0 |
| F10 | **Marca-texto** semitransparente (existe no Lightshot) | P1 |
| F11 | **Paleta de 6 cores fixas:** amarelo, azul, verde, vermelho, preto, branco. Na barra aparece **um único quadradinho** com a cor ativa; clicar nele abre as 6 cores e clicar numa cor fecha a paleta. Sem seletor de cor completo. Padrão: vermelho | P0 |
| F12 | **Espessura do traço** das formas, setas e caneta: ajuste rápido pela roda do mouse durante o uso; valor padrão definido nas configurações | P0 |
| F13 | **Texto:** clicar em qualquer ponto da seleção abre uma caixa e já se digita, como no Lightshot. Enter quebra linha; clicar fora ou Esc finaliza a caixa | P0 |
| F14 | **Tamanho, cor e fundo do texto:** usa a cor ativa da paleta; tamanho ajustável pela roda do mouse; tamanho padrão definido nas configurações. **Fundo do texto** (caixa colorida atrás da letra, para leitura sobre imagens poluídas) é uma opção liga/desliga nas configurações, desligada por padrão | P0 |
| F15 | Toda anotação é um objeto: selecionar, mover, redimensionar, mudar cor, apagar | P0 |
| F16 | Pixelização por retângulo, aplicada na exportação de forma irreversível (pixels originais não ficam no arquivo) | P0 |
| F17 | Passos numerados: cada clique cria um círculo 1, 2, 3…; apagar um renumera os demais | P0 |
| F18 | Desfazer/refazer (Ctrl+Z / Ctrl+Y) | P0 |
| F19 | Ctrl+C ou ícone Copiar: imagem final na área de transferência e fecha | P0 |
| F20 | Ctrl+S ou ícone Salvar: **sempre abre a janela "Salvar como"** do Windows para escolher o local. Não existe pasta padrão. Formato PNG, nome sugerido por data/hora | P0 |
| F21 | Esc (fora de uma caixa de texto) cancela sem deixar rastro | P0 |
| F22 | **Configurações** (arquivo `.json` ao lado do `.exe`, editado por uma janela simples): atalho, iniciar com Windows, espessura padrão do traço, tamanho padrão do texto, cor padrão, fundo do texto liga/desliga | P0 |

A posição fixa das barras (F7) evita uma reclamação comum sobre o Flameshot, cujos botões mudam de lugar ao redor da seleção ([AlternativeTo](https://alternativeto.net/software/flameshot)). O último tamanho e a última cor usados ficam valendo até fechar a captura; a próxima captura volta aos padrões das configurações.

**Atalhos dentro do editor**

| Tecla | Ação |
| --- | --- |
| Ctrl+C | Copiar e fechar |
| Ctrl+S | Abrir "Salvar como" |
| Ctrl+Z / Ctrl+Y | Desfazer / refazer |
| Del | Apagar objeto selecionado |
| Esc | Sair da caixa de texto; fora dela, cancelar |
| Roda do mouse | Aumentar/diminuir espessura ou tamanho do texto |
| Shift (segurando) | Linha a 45°, quadrado, círculo perfeito |
| 1 a 6 | Amarelo, azul, verde, vermelho, preto, branco (sem abrir a paleta) |
| P, L, A, R, E, M, T, B, N | Caneta, Linha, Seta, Retângulo, Elipse, Marca-texto, Texto, Pixelizar, Numerar |

Atalhos de uma letra e de número ficam desligados enquanto uma caixa de texto está aberta, para não trocar de ferramenta no meio da digitação.

## Requisitos não-funcionais e metas

"Na hora" vira um número: **100 ms do atalho até a tela congelada aparecer.** Abaixo de 0,1 s a resposta parece instantânea; acima disso o usuário percebe o atraso (Nielsen 1993, sintetizando Card, Moran e Newell; [UX Tigers](https://www.uxtigers.com/post/time-scales-ux), [feelsfast.fyi](https://feelsfast.vercel.app/concepts/canonical-thresholds)).

As demais metas são **propostas minhas, não benchmarks**. Servem para dar critério de corte; ajuste depois de medir o primeiro protótipo.

| Métrica | Meta | Como medir |
| --- | --- | --- |
| Atalho → seleção visível | ≤ 100 ms | Log com timestamp no hook e no primeiro frame desenhado; 20 execuções, mediana e pior caso |
| Traço acompanha o mouse | ≤ 16 ms por quadro (60 fps) | Contador de frames no modo debug |
| Ctrl+C → imagem na área de transferência | ≤ 200 ms | Timestamp no log |
| RAM parado na bandeja | ≤ 30 MB (proposta) | Gerenciador de Tarefas, coluna Memória |
| CPU parado | \~0% | Gerenciador de Tarefas por 5 min |
| Tamanho do `.exe` | ≤ 10 MB, arquivo único (proposta) | Propriedades do arquivo |
| Conexões de rede | Zero | Monitor de Recursos → aba Rede, durante uso normal |

**Como cumprir os 100 ms:** o processo fica residente na bandeja com a janela de sobreposição já criada e escondida. No atalho, ele captura a tela e só mostra a janela pronta; nada é carregado do disco nesse momento. O que custa caro é abrir um programa do zero, não a captura em si.

**Segurança:** sem rede, sem logs com conteúdo de imagem, sem arquivos temporários. A imagem só vai para disco quando o usuário aperta Ctrl+S.

## Stack técnica

**Decidido: Rust com as APIs nativas do Windows** (crate `windows`, mantida pela Microsoft) e Direct2D para desenhar. Gera um `.exe` único sem runtime para instalar, que é o que os princípios pedem. A tabela abaixo fica como registro das alternativas avaliadas.

| Opção | Prós | Contras | Veredito |
| --- | --- | --- | --- |
| Rust + Win32 + Direct2D | `.exe` único, sem runtime; controle total da janela e do desenho; memória segura | Curva de aprendizado alta se você quiser ler/depurar o código sozinho | **Recomendado** |
| C++ + Win32 + Direct2D | Mesma leveza do Rust | Mais fácil introduzir bugs de memória; sem ganho real sobre Rust aqui | Alternativa |
| C# + WinForms | Código mais fácil de ler; Claude Code domina bem | Native AOT não é suportado oficialmente em WinForms; contornos dependem de biblioteca de terceiros, inclusive para a área de transferência ([Medium, kant2002](https://codevision.medium.com/again-winforms-and-nativeaot-fd068425cc13)). Sem AOT, os colegas precisam do .NET Desktop Runtime instalado | Se você priorizar entender o código |
| Tauri / Electron | Interface em HTML | Carrega um motor de navegador (WebView2/Chromium) só para desenhar uma sobreposição; contradiz "leve" | **Descartado** |

**Blocos técnicos principais**

1. **Atalho global:** `RegisterHotKey`. No Windows 11 o PrtScn abre a Ferramenta de Captura por padrão; o app deve detectar falha no registro e orientar a desligar a opção em Configurações › Acessibilidade › Teclado ([Pureinfotech](https://pureinfotech.com/enable-print-key-take-screenshot-snipping-tool-windows-11)).
2. **Captura:** copiar a área de trabalho virtual inteira (todos os monitores) para um bitmap em memória, ciente de DPI por monitor.
3. **Sobreposição:** janela sem borda, sempre no topo, cobrindo todos os monitores, pré-criada e escondida.
4. **Anotações:** lista de objetos (tipo, pontos, cor, espessura). A imagem final só é "achatada" na hora de copiar/salvar, o que permite editar tudo até lá.
5. **Exportar:** PNG via WIC; área de transferência como bitmap padrão (cola em Slack, Teams, Word, Paint).

## Riscos

| Risco | Impacto | Mitigação |
| --- | --- | --- |
| PrtScn já tomado pela Ferramenta de Captura | Atalho não funciona | Detectar e avisar; atalho alternativo configurável |
| `.exe` sem assinatura nos PCs dos colegas | SmartScreen mostra "Editor desconhecido"; no Windows 11 o Smart App Control pode **bloquear** arquivos sem assinatura e sem reputação ([Microsoft Learn](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)) | Aceitar para uso próprio. Para colegas: avaliar se a TI da empresa permite; certificado de assinatura tem custo anual |
| Desfoque reversível | Dado sensível vaza | Pixelização com blocos grandes e sobrescrita dos pixels na exportação; teste de aceite abaixo |
| Escopo crescer | App deixa de ser leve | Regra: feature nova só entra se as metas continuarem batendo |

## Critérios de aceite da v1

- [ ] Mediana atalho → seleção ≤ 100 ms em 20 execuções
- [ ] Etiqueta de medidas mostra o tamanho exato em pixels do PNG gerado
- [ ] Barras de ícones nunca saem da tela, mesmo com seleção encostada na borda
- [ ] Captura correta em 2 monitores com escalas diferentes (ex.: 100% e 150%)
- [ ] As 6 cores, a espessura e o tamanho do texto funcionam em todas as ferramentas
- [ ] Valores padrão das configurações são aplicados a cada nova captura
- [ ] Todo objeto pode ser movido e apagado depois de desenhado
- [ ] Área pixelizada não é recuperável ao abrir o PNG salvo em um editor
- [ ] Ctrl+C cola corretamente no Teams/Slack, Word e Paint
- [ ] Monitor de Recursos mostra zero conexões de rede do processo

## Roteiro

1. **Protótipo de latência:** bandeja + atalho + tela congelada + seleção tracejada com medidas + Ctrl+C. Medir os 100 ms antes de qualquer outra coisa.
2. **Barras de ícones:** barra vertical e barra horizontal (Copiar, Salvar, Fechar) presas à seleção.
3. **Ferramentas:** caneta, linha, seta, retângulo, elipse, texto, paleta de 6 cores, espessura/tamanho pela roda do mouse, desfazer.
4. **Diferenciais:** objetos editáveis, pixelização, passos numerados, marca-texto.
5. **Acabamento:** salvar, janela de configurações com os padrões, iniciar com Windows, múltiplos monitores/DPI.
6. **Uso real por 2 semanas** antes de passar para os colegas.

## Decisões tomadas

- [x] Linguagem: **Rust**.
- [x] Atalho: **PrtScn**; alternativa **Shift+PrtScn** se o Windows não liberar; configurável.
- [x] Salvamento: **sem pasta padrão**, sempre pergunta o local.
- [x] Distribuição: **só depois do teste pessoal** (etapa 6 do roteiro).
- [x] Cores: **um quadradinho que abre as 6**, para não poluir a barra.
- [x] Formas: **só contorno**, sem preenchimento.
- [x] Fundo do texto: **opção nas configurações**, desligada por padrão.

## Fontes

- [Lightshot – AlternativeTo](https://alternativeto.net/software/lightshot/about)
- [Flameshot alternatives – AlternativeTo](https://alternativeto.net/software/flameshot)
- [LightShot Download 2026 – CodeGenes](https://www.codegenes.net/blog/lightshot-download/)
- [Lightshot Review – AnyMP4](https://www.anymp4.com/recorder/lightshot-review.html)
- [Time Scales of UX – Jakob Nielsen](https://www.uxtigers.com/post/time-scales-ux)
- [The canonical thresholds – feelsfast.fyi](https://feelsfast.vercel.app/concepts/canonical-thresholds)
- [Print Screen e Ferramenta de Captura – Pureinfotech](https://pureinfotech.com/enable-print-key-take-screenshot-snipping-tool-windows-11)
- [WinForms e NativeAOT – kant2002](https://codevision.medium.com/again-winforms-and-nativeaot-fd068425cc13)
- [SmartScreen reputation – Microsoft Learn](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)
