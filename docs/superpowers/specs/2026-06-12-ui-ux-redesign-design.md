# termAI UI/UX Redesign — Design Spec

**Date:** 2026-06-12
**Status:** Draft, pending user approval
**Scope:** Linguagem visual completa do emulador (Rust). Não toca no motor Go.

## Goal

Levar o termAI de uma UI text-grid funcional para uma linguagem visual minimalista, dark-first, com identidade. Referência principal: macOS Terminal.app com chrome fundido (traffic lights dentro do tab strip), tabs equal-width, conteúdo quase preto, com uma única cor de acento magenta (#c44dff) usada com economia.

A direção escolhida ("approach B") mantém o feel minimalista da referência e adiciona um sotaque visual sutil para dar identidade ao produto e tornar visíveis as features de IA sem ocupar espaço permanente.

## Non-goals

- Mudar o motor Go ou o protocolo IPC
- Adicionar features funcionais (reorder de tabs, command palette, sessões persistentes) — vêm depois
- Visual regression testing automatizado — custo > benefício nesse estágio
- Suporte de temas customizados ao chrome — temas existentes continuam controlando apenas a paleta ANSI
- Atalhos de teclado novos (todos os bindings atuais permanecem)

## Design Tokens

Novo módulo `crates/termai-app/src/theme/tokens.rs` centraliza constantes do design system.

### Cores base

| Token | Hex | Uso |
|---|---|---|
| `window_bg` | `#0a0a0a` | Área de conteúdo do terminal |
| `chrome_bg` | `#1c1c1c` | Tab strip, tabs inativas |
| `chrome_bg_active` | `#262626` | Tab ativa |
| `chrome_border` | `#2e2e2e` | Separadores verticais entre tabs, borda inferior do strip, gutter entre panes |
| `text_primary` | `#e6e6e6` | Texto principal, título de tab ativa |
| `text_muted` | `#8a8a8a` | Título de tab inativa, contadores |
| `text_dim` | `#5a5a5a` | Ghost text (autocomplete IA), hints, indicador de conexão |
| `accent` | `#c44dff` | Cursor, borda inferior de tab ativa, borda de pane focado, highlight de match, ações de IA |

### Preset ANSI "termAI Dark" (default)

Calibrado para contraste em `window_bg`:

| ANSI | Hex |
|---|---|
| Preto | `#0a0a0a` |
| Vermelho | `#ff5c57` |
| Verde | `#5af78e` |
| Amarelo | `#f3f99d` |
| Azul | `#57c7ff` |
| Magenta | `#c44dff` (intencionalmente igual ao `accent`) |
| Ciano | `#9aedfe` |
| Branco | `#e6e6e6` |
| Brights | Cada cor base com luminância +10% |

Override pelo usuário via `config.toml` continua funcionando. Os 13 temas existentes ficam como presets opcionais.

### Tipografia

- Família única: JetBrains Mono (já embutida no `termai-renderer`)
- Conteúdo: 14pt
- Chrome (tabs): 12pt — separação visual entre UI e conteúdo

### Spacing

Unidade base `4px`. Tudo é múltiplo: 4, 8, 12, 16, 24.

- Padding da área de conteúdo: `12px` left, `8px` top/right, `4px` bottom
- Altura do tab strip: `36px` + `1px` de borda inferior
- Reserva à esquerda do strip para traffic lights (macOS): `78px`

## Mudança Arquitetural

### Hoje

Tab bar, search bar e AI overlay são renderizadas como `Vec<Vec<RenderCell>>` — fazem parte do mesmo desenho do grid de caracteres do terminal. Limita:

- Alinhamento apenas a múltiplos de cell width/height
- Separadores precisam ser caracteres (pipe `|`)
- Sem cantos arredondados, sem sombras, sem largura fracionária
- Sem hover states ou transições

### Proposta

Novo módulo `crates/termai-renderer/src/ui.rs` com primitivas wgpu de UI desacopladas do grid:

```rust
pub struct RectShape {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],   // RGBA
    pub border_radius: f32,
}

pub struct TextRun {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
}

impl Renderer {
    pub fn draw_ui(&mut self, shapes: &[RectShape], text: &[TextRun]);
}
```

Novo pipeline wgpu com alpha blending, chamado após o draw do grid no mesmo frame. Tab bar, search bar, AI overlay, pane focus border e indicador de IA passam a usar esse pipeline.

Custo estimado: ~200–300 linhas no renderer, +1ms de frame time (orçamento confortável a 60fps).

## Window Chrome

### macOS

Configurar `WindowAttributes` no winit:

- `with_titlebar_transparent(true)`
- `with_fullsize_content_view(true)`

Resultado: traffic lights aparecem dentro da área de desenho do app, sobrepostos no canto superior esquerdo do tab strip. Corner radius da janela é fornecido pelo SO.

### Linux / Windows

Decorações nativas padrão (não tentamos esconder). Tab strip do termAI fica logo abaixo da title bar do SO. Visual ainda funciona, apenas não é "fundido" com os controles como no macOS.

## Tab Bar

Módulo novo: `crates/termai-app/src/ui/tab_bar.rs`. Substitui `build_tab_bar_cells()`.

### Layout

```
┌──────────────────────────────────────────────────────────────────┐
│ [● ● ●]        ~              │       ~              │       ~   │   strip 36px, bg chrome_bg
├──────────────────────────────────────────────────────────────────┤   border 1px chrome_border
│                                                                  │
│ viniciusaguiar@192 ~ %                                           │   conteúdo bg window_bg
```

- macOS: reserva `78px` à esquerda para traffic lights. Linux/Windows: `0px`.
- Tabs ocupam o restante divididas igualmente: `tab_width = (strip_width - reserved_left) / tab_count`
- Largura ideal por tab: entre `120px` (mín) e `240px` (máx)
- Se `tab_count × 120 > strip_width - reserved_left`: tabs encolhem proporcionalmente até `60px` mínimo absoluto, com truncate por ellipsis no título. Scroll horizontal de tabs fica fora de escopo nesse round.
- Se sobra espaço com largura máxima: strip sobra à direita em `chrome_bg`.

### Rendering por tab

- Bg: `chrome_bg` (inativa) ou `chrome_bg_active` (ativa)
- Título: cwd da pane focada, path-shortened se passar de 20 chars (`~/proj/termai` → `~/p/termai`); ellipsis se ainda passar
- Cor do título: `text_muted` (inativa) ou `text_primary` (ativa), centralizado horizontal e vertical
- Separadores: linha vertical `1px` em `chrome_border` entre cada par de tabs (não desenha entre área dos traffic lights e primeira tab)
- Tab ativa: linha `2px` na borda inferior em `accent` — único detalhe colorido do strip

### Estados

- Hover (mouse sobre tab inativa): bg interpola para `#222` em `120ms`
- Click: switch tab (mantém `TabBar::switch_to`)
- Cmd+T: nova tab (mantém)
- Cmd+W: fecha tab ativa (mantém); botão `×` aparece no hover, à direita do título, em `text_dim`
- Cmd+Shift+] / [: próxima/anterior (mantém)

### Connection indicator (IA)

- Posição: canto direito do strip, `8px` do edge
- Tamanho: `8×8px`
- Estados:
  - Conectada: círculo cheio em `text_dim`
  - Desconectada: círculo outline `1px` em `text_dim`
  - Analisando agora: círculo cheio em `accent`, pulsando (opacity 0.5↔1.0, 1s)
- Sem clique, sem tooltip (anotado como futuro)

### Hit testing

Mouse position vira tab index via `(x - reserved_left) / tab_width`. Reordenar tabs por drag fica fora de escopo.

## Content Area

### Padding e tipografia

- Padding: `12px` left, `8px` top, `8px` right, `4px` bottom
- Cell metrics: JetBrains Mono 14pt — cell ≈ 20×8.4px. Line gap embutido.
- Anti-aliasing: subpixel via `ab_glyph`. Verificar hinting após troca de monitor (bug latente; anotado para fix no Step 8).

### Cursor

- Estilo padrão: bloco sólido em `accent`, 100% opacidade quando pane focada
- Blink: 530ms (mantém), suaviza com fade (opacity 1.0 ↔ 0.4, sine) em vez de on/off bruto
- Pane não focada: cursor vira outline `1px` em `accent`, sem blink
- Configurável: `cursor.style = "block" | "bar" | "underline"` no `config.toml`. Default `block`.

### Seleção

- Bg: `accent` com 25% alpha (`#c44dff40`)
- Fg: preserva cor original do texto (não inverte)
- Requer composição alpha correta no shader: o BG real da célula precisa ser passado para o vertex, o que já é suportado pela struct `Vertex` existente

### Ghost text (autocomplete IA)

- Cor: `text_dim`
- Sem itálico (evita conflito com texto via ANSI italic)
- Tecla Tab aceita, Esc descarta (mantém)

### Pane focus border

- Borda `1px` em `accent` ao redor da pane focada, inset `0` (cola no perímetro)
- Pane não focada: sem borda
- Gutter entre panes: `1px` em `chrome_border` como divisor
- Renderiza no pipeline UI, não no grid

## Overlays

### Search bar (redesenho de posição e estilo)

Hoje: linha de células no fundo da janela. Proposta: componente flutuante no canto superior direito da área de conteúdo.

```
                                          ┌──────────────────────┐
                                          │ ⌕  termAI       3/17 │   32px altura, 280px largura
                                          └──────────────────────┘
```

- Bg: `chrome_bg_active` com borda `1px` em `chrome_border`, `border_radius: 6px`
- Shadow: `0 4px 12px rgba(0,0,0,0.4)` — única sombra do app
- Ícone esquerda: lupa (Unicode `⌕` U+2315, ou glyph dedicado se hinting ruim)
- Input: `text_primary`, prompt em `text_dim` ("Buscar..."). Cursor magenta.
- Contador direita: `3/17` em `text_muted`
- Highlight de matches no conteúdo: `accent` com 35% alpha para todos os matches, 70% alpha para o match atual
- Posição: float a `8px` do topo e da direita do conteúdo, sobrepondo (não empurra)
- Keys: Cmd+F abre, Esc fecha, Enter próximo, Shift+Enter anterior (mantém)

### AI overlay

Mesma posição atual (fundo da pane focada), nova linguagem:

- Container: bg `chrome_bg` com `border-top 2px accent`, sem borda lateral, sem cantos arredondados (cola na largura da pane)
- Padding: `12px` vertical, `16px` horizontal
- Tipografia:
  - Título: `text_primary`, bold, 14pt
  - Descrição: `text_muted`, 13pt, 1 linha (truncate com ellipsis)
  - Ações: linha `[1] Carregar nvm e usar a versão` — número em `accent`, label em `text_primary`. Risk badge à direita:
    - `low`: ponto verde `6px`
    - `medium`: ponto amarelo `6px`
    - `high`: ponto vermelho `6px` + texto "high" em `text_muted`
- Auto-dismiss: 10s (mantém), fade out 200ms
- Manual dismiss: Esc. Atalhos: 1, 2, 3 para executar ações.

### Z-order de overlays

De baixo para cima:

1. Conteúdo do terminal
2. Borda da pane focada
3. AI overlay (na pane)
4. Search bar (na janela)
5. (futuro: command palette)

Search bar e AI overlay não competem visualmente — se ambos estão presentes, AI overlay continua mas search aparece acima. Aceitável: search é raro, AI é resposta a comando.

## Migration Order

8 steps. Cada um é um PR/commit que compila, roda e tem entrega visual descritível. Sem big-bang.

| Step | Conteúdo | Visual delta |
|---|---|---|
| 1 | Design tokens em `theme/tokens.rs`, preset ANSI "termAI Dark", refator de hardcodes | Mínimo |
| 2 | Pipeline UI novo em `termai-renderer/src/ui.rs` (não consumido ainda) | Zero |
| 3 | `fullsize_content_view` + reserva de 78px (macOS) | Traffic lights sobre tab strip (pode quebrar layout temporariamente, ok dentro do step) |
| 4 | Nova tab bar (`ui/tab_bar.rs`) usando pipeline UI: equal-width, separadores, acento na ativa, hover, connection indicator placeholder | App já parece com a referência |
| 5 | Cursor fade + outline, pane focus border, seleção alpha, ghost text dim | Mais coeso |
| 6 | Search bar flutuante top-right, highlights alpha | Search vira componente UI |
| 7 | AI overlay refinado + connection indicator conectado ao estado real do `ai_client` | IA integrada visualmente |
| 8 | Polish: anti-aliasing multi-monitor, throttle de hover, smoke test Linux/Windows | Cleanup |

Steps 1–2 são infra (sem visual). Steps 3–4 são o grosso. 5–7 refinam. 8 fecha.

## Testing

### Visual regression — não automatizar

Snapshot testing para wgpu é caro de configurar e manter. Projeto é pequeno. Em vez disso:

- Checklist de QA manual por step, documentado no PR description
- Screenshots antes/depois anexados ao PR
- Rodar `vttest` periodicamente para garantir que o core VT não regrediu (ortogonal ao UI)

### Unit tests (Rust)

- `theme/tokens.rs`: constantes — pulamos
- `ui::tab_bar::layout()`: rects corretos para N tabs com largura W
- `ui::tab_bar::hit_test(x, y)`: índice da tab clicada
- `ui::rect_shape`: geometria (intersect, contains)
- Renderer pipeline UI: smoke test em contexto headless — desenha 1 rect, verifica buffer não vazio

### Integration / Manual

- Sem integration test novo nessa rodada
- Cenário smoke por PR: abrir, dividir pane, mudar tab, buscar, gerar erro, dismissar IA
- Plataforma de desenvolvimento: macOS (chrome integrado é o caminho feliz)
- Linux/Windows: testar ao final da rodada, antes do release. Se chrome nativo do SO criar conflito visual, ajusta no Step 8.

## Riscos & mitigações

| Risco | Mitigação |
|---|---|
| `fullsize_content_view` tem quirks no winit 0.30 | Fallback: title bar nativa visível, chrome do termAI abaixo. Não bloqueia. |
| Pipeline UI extra adiciona ~1ms de frame time | Aceitável dentro do orçamento de 60fps |
| Migração tab bar grid → UI quebra `handle_tab_bar_click` | Step 4 reescreve junto; revisão cuidadosa do hit test no PR |
| Override de tema do usuário não cobre chrome | Decisão consciente: temas só controlam ANSI nesse round. Chrome tematizável fica como feature futura. |

## Out of scope (parking lot)

- Tematização de chrome via config do usuário
- Drag para reordenar tabs
- Command palette (Cmd+K)
- Tooltips nos indicadores
- Status bar inferior (foi a abordagem C, rejeitada)
- Window decorations próprias no Linux/Windows
- Visual regression testing automatizado
- Sessões persistentes / restauração de tabs
