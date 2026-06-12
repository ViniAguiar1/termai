# termAI UI/UX Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar a linguagem visual minimalista dark-first descrita em `docs/superpowers/specs/2026-06-12-ui-ux-redesign-design.md` — tab bar redesenhada com chrome fundido (macOS), cursor magenta, search bar flutuante, AI overlay refinado, e indicador discreto de conexão IA.

**Architecture:** Tudo no crate Rust. O renderer existente já suporta retângulos coloridos com alpha (`build_rect`, `build_divider`) e drawing fora do grid, então **não criamos um pipeline wgpu novo** — adicionamos um segundo atlas de glyphs em 12pt para chrome text e helpers para texto livre. Tab bar, search bar e AI overlay migram de células do grid para shapes + texto livre. Mudança é cirúrgica em cada componente, comportamento gera commits independentes.

**Tech Stack:** Rust 2024, wgpu 24, winit 0.30, ab_glyph 0.2. Sem dependências novas.

---

## File Structure

### Created
- `crates/termai-app/src/theme/mod.rs` — re-exporta tokens
- `crates/termai-app/src/theme/tokens.rs` — constantes de design (cores, spacing, font sizes)
- `crates/termai-app/src/ui/mod.rs` — módulo container pra componentes UI
- `crates/termai-app/src/ui/tab_bar.rs` — renderer da tab bar (substitui `build_tab_bar_cells`)
- `crates/termai-app/src/ui/search_bar.rs` — renderer da search bar flutuante (substitui `build_search_bar_cells`)
- `crates/termai-app/src/ui/ai_overlay.rs` — renderer do AI overlay (substitui `build_ai_overlay_cells`)
- `crates/termai-app/src/ui/connection_indicator.rs` — bolinha 8×8 no canto direito do strip
- `crates/termai-app/src/ui/path_shorten.rs` — utilitário pra encurtar paths (`~/proj/termai` → `~/p/termai`)

### Modified
- `crates/termai-renderer/src/lib.rs` — adiciona segundo atlas (12pt), helpers `build_text_run`, `build_rect_outline`
- `crates/termai-renderer/src/atlas.rs` — não muda (já suporta criar atlas em qualquer size)
- `crates/termai-app/src/colors.rs` — adiciona preset "termAI Dark", marca como default
- `crates/termai-app/src/main.rs` — substitui `build_tab_bar_cells`, `build_search_bar_cells`, `build_ai_overlay_cells`; ajusta layout do conteúdo; cursor fade; pane border; selection alpha; ghost text dim; window chrome (macOS)
- `crates/termai-app/src/pane.rs` — adiciona tracking de cwd na Pane
- `crates/termai-app/src/ai.rs` — expõe `is_connected()` e `is_analyzing()` no AiClient
- `crates/termai-app/src/config.rs` — adiciona `cursor.style` config
- `crates/termai-app/Cargo.toml` — sem mudanças

### Convention
Cada task termina com `cargo check && cargo test -p termai-core -p termai-renderer -p termai-app` passando e um commit. Mensagens seguem o padrão dos commits existentes (feat:, fix:, refactor:).

---

## Phase 1 — Foundation

### Task 1: Module scaffolding + design tokens

**Files:**
- Create: `crates/termai-app/src/theme/mod.rs`
- Create: `crates/termai-app/src/theme/tokens.rs`
- Modify: `crates/termai-app/src/main.rs:1-6` (declarar módulo `theme`)

- [ ] **Step 1: Criar `theme/mod.rs`**

Conteúdo:
```rust
pub mod tokens;
```

- [ ] **Step 2: Criar `theme/tokens.rs` com a paleta**

Conteúdo completo:
```rust
//! Design tokens for the termAI visual language.
//!
//! All colors are normalized RGBA in [0.0, 1.0].
//! Spacing unit is 4 pixels.

pub const WINDOW_BG: [f32; 4] = rgb(0x0a, 0x0a, 0x0a);
pub const CHROME_BG: [f32; 4] = rgb(0x1c, 0x1c, 0x1c);
pub const CHROME_BG_ACTIVE: [f32; 4] = rgb(0x26, 0x26, 0x26);
pub const CHROME_BORDER: [f32; 4] = rgb(0x2e, 0x2e, 0x2e);
pub const TEXT_PRIMARY: [f32; 4] = rgb(0xe6, 0xe6, 0xe6);
pub const TEXT_MUTED: [f32; 4] = rgb(0x8a, 0x8a, 0x8a);
pub const TEXT_DIM: [f32; 4] = rgb(0x5a, 0x5a, 0x5a);
pub const ACCENT: [f32; 4] = rgb(0xc4, 0x4d, 0xff);

pub const UNIT: f32 = 4.0;

// Spacing
pub const CONTENT_PADDING_LEFT: f32 = 12.0;
pub const CONTENT_PADDING_TOP: f32 = 8.0;
pub const CONTENT_PADDING_RIGHT: f32 = 8.0;
pub const CONTENT_PADDING_BOTTOM: f32 = 4.0;

// Tab strip
pub const TAB_STRIP_HEIGHT: f32 = 36.0;
pub const TAB_STRIP_BORDER: f32 = 1.0;
pub const TAB_MIN_WIDTH: f32 = 120.0;
pub const TAB_MIN_WIDTH_ABSOLUTE: f32 = 60.0;
pub const TAB_MAX_WIDTH: f32 = 240.0;
pub const TAB_ACTIVE_ACCENT_HEIGHT: f32 = 2.0;

// macOS: pixel reserve for traffic lights on the left of the strip.
#[cfg(target_os = "macos")]
pub const TRAFFIC_LIGHTS_RESERVE: f32 = 78.0;
#[cfg(not(target_os = "macos"))]
pub const TRAFFIC_LIGHTS_RESERVE: f32 = 0.0;

// Connection indicator (right side of strip)
pub const CONNECTION_INDICATOR_SIZE: f32 = 8.0;
pub const CONNECTION_INDICATOR_RIGHT_PAD: f32 = 8.0;

// Typography
pub const FONT_SIZE_CONTENT: f32 = 14.0;
pub const FONT_SIZE_CHROME: f32 = 12.0;

// Alpha
pub const SELECTION_ALPHA: f32 = 0.25;
pub const SEARCH_MATCH_ALPHA: f32 = 0.35;
pub const SEARCH_CURRENT_MATCH_ALPHA: f32 = 0.70;

// Animation
pub const HOVER_TRANSITION_MS: u128 = 120;
pub const OVERLAY_FADE_MS: u128 = 200;
pub const CURSOR_BLINK_MS: u128 = 530;
pub const CURSOR_FADE_MIN: f32 = 0.4;
pub const PULSE_PERIOD_MS: u128 = 1000;

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub const fn with_alpha(color: [f32; 4], alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_color_matches_spec() {
        // #c44dff = (196, 77, 255)
        assert_eq!(ACCENT, [196.0 / 255.0, 77.0 / 255.0, 255.0 / 255.0, 1.0]);
    }

    #[test]
    fn with_alpha_preserves_rgb() {
        let result = with_alpha(ACCENT, 0.25);
        assert_eq!(result[0], ACCENT[0]);
        assert_eq!(result[1], ACCENT[1]);
        assert_eq!(result[2], ACCENT[2]);
        assert_eq!(result[3], 0.25);
    }
}
```

- [ ] **Step 3: Declarar módulo no `main.rs`**

Edit `crates/termai-app/src/main.rs:1-6` — adicionar linha:
```rust
mod ai;
mod colors;
mod config;
mod pane;
mod tab;
mod theme;  // <-- nova linha
```

- [ ] **Step 4: Rodar testes e check**

```bash
cargo check -p termai-app
cargo test -p termai-app theme::tokens
```
Expected: 2 testes passam, sem warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/termai-app/src/theme crates/termai-app/src/main.rs
git commit -m "feat(theme): add design tokens module"
```

---

### Task 2: Renderer — segundo atlas em 12pt e helpers de texto livre

**Files:**
- Modify: `crates/termai-renderer/src/lib.rs` (adicionar `chrome_atlas`, métodos `build_text_run`, `build_rect_outline`, `chrome_cell_size`)

- [ ] **Step 1: Adicionar campo `chrome_atlas` na struct `Renderer`**

Edit `crates/termai-renderer/src/lib.rs:38-52` — adicionar campo:
```rust
pub struct Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    atlas: GlyphAtlas,
    chrome_atlas: GlyphAtlas,
    chrome_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    pub clear_color: [f32; 4],
}
```

- [ ] **Step 2: Construir o `chrome_atlas` no `new()`**

Edit `Renderer::new()` em `crates/termai-renderer/src/lib.rs`. Após a criação do atlas principal (~linha 111), adicionar:

```rust
// Chrome atlas at smaller font size for UI text (tabs, search bar, AI overlay)
let chrome_font_size = 12.0 * scale_factor;
let chrome_atlas = GlyphAtlas::new(FONT_BYTES, chrome_font_size);

let chrome_atlas_texture = device.create_texture_with_data(
    &queue,
    &wgpu::TextureDescriptor {
        label: Some("chrome-glyph-atlas"),
        size: wgpu::Extent3d {
            width: chrome_atlas.texture_width,
            height: chrome_atlas.texture_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    },
    wgpu::util::TextureDataOrder::LayerMajor,
    &chrome_atlas.texture_data,
);

let chrome_atlas_view = chrome_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
let chrome_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    address_mode_u: wgpu::AddressMode::ClampToEdge,
    address_mode_v: wgpu::AddressMode::ClampToEdge,
    mag_filter: wgpu::FilterMode::Linear,
    min_filter: wgpu::FilterMode::Linear,
    ..Default::default()
});

let chrome_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("chrome-bind-group"),
    layout: &bind_group_layout,
    entries: &[
        wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(&chrome_atlas_view),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::Sampler(&chrome_atlas_sampler),
        },
    ],
});
```

Adicionar `chrome_atlas` e `chrome_bind_group` ao struct literal de retorno do `new()` (após `atlas` e `bind_group`).

- [ ] **Step 3: Adicionar `chrome_cell_size()` e `build_text_run()`**

Adicionar como métodos públicos do `Renderer` (após `cell_size()` ~linha 376):

```rust
/// Cell dimensions for the chrome (UI) atlas in pixels.
pub fn chrome_cell_size(&self) -> (f32, f32) {
    (self.chrome_atlas.cell_width, self.chrome_atlas.cell_height)
}

/// Draw a free-positioned text run (not aligned to the grid) using the chrome atlas.
/// Returns the pixel width consumed by the text.
pub fn build_text_run(
    &mut self,
    text: &str,
    x: f32,
    y: f32,
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
) -> f32 {
    let (cell_w, _) = self.chrome_cell_size();
    let mut cursor_x = x;

    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += cell_w;
            continue;
        }
        if let Some(glyph) = self.chrome_atlas.get_or_insert(ch) {
            let glyph = *glyph;
            let x0 = cursor_x + glyph.offset_x;
            let y0 = y + glyph.offset_y;
            let x1 = x0 + glyph.width;
            let y1 = y0 + glyph.height;
            push_quad(
                vertices,
                x0, y0, x1, y1,
                [glyph.uv_x, glyph.uv_y],
                [glyph.uv_x + glyph.uv_w, glyph.uv_y + glyph.uv_h],
                color,
                [0.0; 4],
                0.0,
            );
            cursor_x += cell_w;
        }
    }

    cursor_x - x
}

/// Measure the pixel width a text run would consume in the chrome atlas, without drawing it.
pub fn measure_chrome_text(&self, text: &str) -> f32 {
    let (cell_w, _) = (self.chrome_atlas.cell_width, self.chrome_atlas.cell_height);
    text.chars().count() as f32 * cell_w
}

/// Build a 1px outline rectangle (4 thin rects).
pub fn build_rect_outline(
    &self,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
) {
    self.build_rect(x, y, w, thickness, color, vertices);              // top
    self.build_rect(x, y + h - thickness, w, thickness, color, vertices); // bottom
    self.build_rect(x, y, thickness, h, color, vertices);              // left
    self.build_rect(x + w - thickness, y, thickness, h, color, vertices); // right
}
```

- [ ] **Step 4: Atualizar `submit_frame()` para usar dois bind groups**

O frame agora tem duas passadas de draw: uma com `bind_group` (atlas principal) e outra com `chrome_bind_group` (atlas de chrome). Refatorar para receber vertices separados:

Substituir `submit_frame()` em `crates/termai-renderer/src/lib.rs:534-584` por:

```rust
/// Render pre-built vertices to the screen.
///
/// `main_vertices` use the main (content) atlas.
/// `chrome_vertices` use the chrome atlas.
pub fn submit_frame(
    &self,
    main_vertices: &[Vertex],
    chrome_vertices: &[Vertex],
) -> Result<(), wgpu::SurfaceError> {
    let output = self.surface.get_current_texture()?;
    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

    let main_vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("main-vertex-buffer"),
        contents: bytemuck::cast_slice(main_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let chrome_vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("chrome-vertex-buffer"),
        contents: bytemuck::cast_slice(chrome_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render-encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: self.clear_color[0] as f64,
                        g: self.clear_color[1] as f64,
                        b: self.clear_color[2] as f64,
                        a: self.clear_color[3] as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        render_pass.set_pipeline(&self.pipeline);

        if !main_vertices.is_empty() {
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, main_vb.slice(..));
            render_pass.draw(0..main_vertices.len() as u32, 0..1);
        }

        if !chrome_vertices.is_empty() {
            render_pass.set_bind_group(0, &self.chrome_bind_group, &[]);
            render_pass.set_vertex_buffer(0, chrome_vb.slice(..));
            render_pass.draw(0..chrome_vertices.len() as u32, 0..1);
        }
    }

    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}
```

Também atualizar `render()` (linha 527) para manter API atual:
```rust
pub fn render(&mut self, cells: &[Vec<RenderCell>]) -> Result<(), wgpu::SurfaceError> {
    let mut vertices: Vec<Vertex> = Vec::new();
    self.build_vertices(cells, 0.0, 0.0, &mut vertices);
    self.submit_frame(&vertices, &[])
}
```

- [ ] **Step 5: Atualizar caller em `main.rs`**

Em `crates/termai-app/src/main.rs:1419` (a chamada de `submit_frame(&vertices)`), trocar para `submit_frame(&vertices, &[])`. Por enquanto chrome_vertices fica vazio — será populado nas tasks seguintes.

- [ ] **Step 6: Adicionar testes de geometria**

Adicionar ao final de `crates/termai-renderer/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_renderer_geometry_test() {
        // Smoke test for push_quad: 6 vertices per rect.
        let mut v: Vec<Vertex> = Vec::new();
        push_quad(&mut v, 0.0, 0.0, 10.0, 10.0, [0.0; 2], [1.0; 2], [1.0; 4], [0.0; 4], 1.0);
        assert_eq!(v.len(), 6);
    }

    #[test]
    fn push_quad_emits_six_vertices() {
        dummy_renderer_geometry_test();
    }
}
```

(Note: testes de pipeline real precisariam de contexto wgpu headless — fora de escopo. Só smoke test de geometria por ora.)

- [ ] **Step 7: Rodar check e testes**

```bash
cargo check -p termai-renderer -p termai-app
cargo test -p termai-renderer
```
Expected: smoke test passa, no warnings novos no app (app já chama nova API).

- [ ] **Step 8: Smoke test visual — app continua rodando**

```bash
cargo run --release
```
Expected: termAI abre normalmente. Comportamento idêntico ao anterior (chrome atlas existe mas não é usado ainda).

- [ ] **Step 9: Commit**

```bash
git add crates/termai-renderer/src/lib.rs crates/termai-app/src/main.rs
git commit -m "feat(renderer): add chrome atlas and free-text helpers"
```

---

### Task 3: ANSI preset "termAI Dark" como default

**Files:**
- Modify: `crates/termai-app/src/colors.rs`

- [ ] **Step 1: Localizar onde os temas são definidos**

Ler `crates/termai-app/src/colors.rs` inteiro para encontrar onde `DEFAULT` é definido.

```bash
grep -n "pub static DEFAULT\|pub const DEFAULT\|pub static.*Theme\|name: \"" crates/termai-app/src/colors.rs
```

- [ ] **Step 2: Adicionar tema "termAI Dark"**

Adicionar ao final de `crates/termai-app/src/colors.rs` (antes do `#[cfg(test)]` se houver):

```rust
/// termAI default dark theme — calibrated for the redesigned visual language.
pub static TERMAI_DARK: Theme = Theme {
    name: "termAI Dark",
    bg: [0x0a as f32 / 255.0, 0x0a as f32 / 255.0, 0x0a as f32 / 255.0, 1.0],
    fg: [0xe6 as f32 / 255.0, 0xe6 as f32 / 255.0, 0xe6 as f32 / 255.0, 1.0],
    cursor: [0xc4 as f32 / 255.0, 0x4d as f32 / 255.0, 0xff as f32 / 255.0, 1.0],
    selection: [0xc4 as f32 / 255.0, 0x4d as f32 / 255.0, 0xff as f32 / 255.0, 0.25],
    ansi: [
        // Normal
        [0x0a as f32 / 255.0, 0x0a as f32 / 255.0, 0x0a as f32 / 255.0, 1.0],  // black
        [0xff as f32 / 255.0, 0x5c as f32 / 255.0, 0x57 as f32 / 255.0, 1.0],  // red
        [0x5a as f32 / 255.0, 0xf7 as f32 / 255.0, 0x8e as f32 / 255.0, 1.0],  // green
        [0xf3 as f32 / 255.0, 0xf9 as f32 / 255.0, 0x9d as f32 / 255.0, 1.0],  // yellow
        [0x57 as f32 / 255.0, 0xc7 as f32 / 255.0, 0xff as f32 / 255.0, 1.0],  // blue
        [0xc4 as f32 / 255.0, 0x4d as f32 / 255.0, 0xff as f32 / 255.0, 1.0],  // magenta
        [0x9a as f32 / 255.0, 0xed as f32 / 255.0, 0xfe as f32 / 255.0, 1.0],  // cyan
        [0xe6 as f32 / 255.0, 0xe6 as f32 / 255.0, 0xe6 as f32 / 255.0, 1.0],  // white
        // Bright (+10% luminance, clamped)
        [0x33 as f32 / 255.0, 0x33 as f32 / 255.0, 0x33 as f32 / 255.0, 1.0],  // br black
        [0xff as f32 / 255.0, 0x7c as f32 / 255.0, 0x77 as f32 / 255.0, 1.0],
        [0x7a as f32 / 255.0, 0xff as f32 / 255.0, 0xae as f32 / 255.0, 1.0],
        [0xff as f32 / 255.0, 0xff as f32 / 255.0, 0xbd as f32 / 255.0, 1.0],
        [0x77 as f32 / 255.0, 0xe7 as f32 / 255.0, 0xff as f32 / 255.0, 1.0],
        [0xe4 as f32 / 255.0, 0x6d as f32 / 255.0, 0xff as f32 / 255.0, 1.0],
        [0xba as f32 / 255.0, 0xff as f32 / 255.0, 0xff as f32 / 255.0, 1.0],
        [0xff as f32 / 255.0, 0xff as f32 / 255.0, 0xff as f32 / 255.0, 1.0],  // br white
    ],
};
```

- [ ] **Step 3: Trocar o default**

Encontrar a linha em `colors.rs` onde `DEFAULT` é exportado (provavelmente algo como `pub static DEFAULT: Theme = ...`) e fazer `DEFAULT` apontar para `TERMAI_DARK`:

```rust
pub static DEFAULT: Theme = TERMAI_DARK.clone();
```

Se `Theme` não tem `Clone` const, criar referência:
```rust
pub fn default_theme() -> Theme {
    TERMAI_DARK.clone()
}
```

E atualizar callers em `main.rs:123`:
```rust
theme: colors::TERMAI_DARK.clone(),
```

(Confirmar pelo grep do step 1 qual padrão o código usa hoje.)

- [ ] **Step 4: Verificar `cargo check`**

```bash
cargo check -p termai-app
```
Expected: passa.

- [ ] **Step 5: Smoke test visual**

```bash
cargo run --release
```
Expected: terminal abre com bg quase preto, cursor magenta, paleta ANSI nova. Tab bar ainda usa estilo antigo (ainda não migrada). Diff visual: cores levemente diferentes do antes.

- [ ] **Step 6: Commit**

```bash
git add crates/termai-app/src/colors.rs crates/termai-app/src/main.rs
git commit -m "feat(theme): make termAI Dark the default ANSI preset"
```

---

## Phase 2 — Chrome

### Task 4: Window chrome — `fullsize_content_view` no macOS

**Files:**
- Modify: `crates/termai-app/src/main.rs` (na função onde `WindowAttributes` é criado)

- [ ] **Step 1: Localizar criação da window**

```bash
grep -n "WindowAttributes\|create_window\|with_title" crates/termai-app/src/main.rs
```

- [ ] **Step 2: Adicionar config macOS-specific**

Adicionar imports no topo do `main.rs`:
```rust
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
```

Onde `WindowAttributes::default()` é configurado, adicionar:
```rust
let mut attrs = Window::default_attributes()
    .with_title("termAI");

#[cfg(target_os = "macos")]
{
    attrs = attrs
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true);
}
```

(Adaptar à API real do código — o snippet acima é guia, não cópia literal.)

- [ ] **Step 3: Verificar cargo check**

```bash
cargo check -p termai-app
```

- [ ] **Step 4: Smoke test visual no macOS**

```bash
cargo run --release
```
Expected: traffic lights aparecem sobre a área de conteúdo (vão se sobrepor à tab bar atual temporariamente — sabido). Comportamento de drag/resize da janela mantido.

- [ ] **Step 5: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(chrome): integrate macOS traffic lights into content area"
```

---

### Task 5: Util `path_shorten` para títulos de tab

**Files:**
- Create: `crates/termai-app/src/ui/mod.rs`
- Create: `crates/termai-app/src/ui/path_shorten.rs`
- Modify: `crates/termai-app/src/main.rs:1-6` (declarar `ui`)

- [ ] **Step 1: Criar módulo ui**

`crates/termai-app/src/ui/mod.rs`:
```rust
pub mod path_shorten;
```

- [ ] **Step 2: Escrever testes primeiro**

`crates/termai-app/src/ui/path_shorten.rs`:
```rust
//! Path shortening for tab titles.
//!
//! `/Users/vini/code/termai` → `~/code/termai` (home expansion only) if ≤ max chars.
//! Otherwise shortens middle segments to their first letter: `~/code/termai` → `~/c/termai`.
//! Final fallback: ellipsis truncation `~/code/te...`.

use std::path::Path;

pub fn shorten<P: AsRef<Path>>(path: P, home: Option<&Path>, max_chars: usize) -> String {
    let path = path.as_ref();
    let mut s = path.to_string_lossy().into_owned();

    if let Some(home) = home {
        if let Ok(rel) = path.strip_prefix(home) {
            s = if rel.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rel.to_string_lossy())
            };
        }
    }

    if s.chars().count() <= max_chars {
        return s;
    }

    // Shorten middle segments to first char.
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() > 2 {
        let last = parts.last().copied().unwrap_or("");
        let mut acc = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                acc.push_str(part);
            } else if i == parts.len() - 1 {
                acc.push('/');
                acc.push_str(last);
            } else if !part.is_empty() {
                acc.push('/');
                acc.push(part.chars().next().unwrap());
            }
        }
        if acc.chars().count() <= max_chars {
            return acc;
        }
        s = acc;
    }

    // Final fallback: ellipsis truncate.
    if s.chars().count() > max_chars {
        let kept: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        return format!("{}…", kept);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn home_replaced_with_tilde() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(shorten("/Users/vini", Some(&home), 20), "~");
        assert_eq!(shorten("/Users/vini/code", Some(&home), 20), "~/code");
    }

    #[test]
    fn short_path_unchanged() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(shorten("/Users/vini/code/termai", Some(&home), 20), "~/code/termai");
    }

    #[test]
    fn middle_segments_shortened_when_over_max() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(
            shorten("/Users/vini/code/projects/termai/crates", Some(&home), 20),
            "~/c/p/termai/crates"
        );
    }

    #[test]
    fn ellipsis_when_still_too_long() {
        let home = PathBuf::from("/Users/vini");
        let result = shorten(
            "/Users/vini/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some(&home),
            10,
        );
        assert!(result.ends_with('…'));
        assert!(result.chars().count() == 10);
    }

    #[test]
    fn no_home_match_keeps_absolute_path() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(shorten("/etc/hosts", Some(&home), 20), "/etc/hosts");
    }
}
```

- [ ] **Step 3: Rodar testes — devem falhar (módulo não declarado)**

```bash
cargo test -p termai-app path_shorten
```
Expected: erro de compilação ou testes inexistentes.

- [ ] **Step 4: Declarar módulo `ui` no `main.rs`**

Edit `crates/termai-app/src/main.rs:1-6`:
```rust
mod ai;
mod colors;
mod config;
mod pane;
mod tab;
mod theme;
mod ui;  // <-- nova linha
```

- [ ] **Step 5: Rodar testes**

```bash
cargo test -p termai-app path_shorten
```
Expected: 5 testes passam.

- [ ] **Step 6: Commit**

```bash
git add crates/termai-app/src/ui crates/termai-app/src/main.rs
git commit -m "feat(ui): add path_shorten utility for tab titles"
```

---

### Task 6: Tracking de cwd na Pane

**Files:**
- Modify: `crates/termai-app/src/pane.rs`

- [ ] **Step 1: Adicionar campo `cwd` na `Pane`**

Editar `crates/termai-app/src/pane.rs`. Localizar a struct `Pane`:
```bash
grep -n "pub struct Pane" crates/termai-app/src/pane.rs
```

Adicionar campo `cwd: Option<std::path::PathBuf>` à struct (logo após `pub terminal: Terminal` ou equivalente).

Inicializar como `None` no `Pane::new()`.

- [ ] **Step 2: Capturar cwd no momento do spawn da PTY**

Onde o shell é spawnado (provavelmente em `Pane::new` ou via `termai-pty`), capturar `std::env::current_dir()` e armazenar como cwd inicial.

- [ ] **Step 3: Atualizar cwd via OSC 7 (se o shell emitir)**

Shells modernos (zsh, fish) emitem `OSC 7;file://hostname/path` para reportar cwd. Verificar se `termai-core` já parseia OSC. Se sim, expor um callback ou um getter que `Pane` polla a cada poll do PTY.

Se OSC 7 não está parseado: adicionar handling mínimo em `termai-core/src/lib.rs` — só capturar a string entre `OSC 7;` e `ST`/`BEL`, descartar prefixo `file://hostname`, e expor como `terminal.cwd: Option<PathBuf>`.

```bash
grep -n "OSC\|osc_dispatch" crates/termai-core/src/lib.rs
```

Se já tem handler de OSC, adicionar branch para `7`. Se não tem, implementar:
```rust
fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
    if let Some(first) = params.first() {
        if let Ok(code) = std::str::from_utf8(first) {
            if code == "7" {
                if let Some(uri_bytes) = params.get(1) {
                    if let Ok(uri) = std::str::from_utf8(uri_bytes) {
                        // file://hostname/path -> /path
                        if let Some(path_start) = uri.find("//").and_then(|i| uri[i+2..].find('/').map(|j| i + 2 + j)) {
                            self.cwd = Some(std::path::PathBuf::from(&uri[path_start..]));
                        }
                    }
                }
            }
        }
    }
}
```

(Adaptar para o trait existente do `vte::Perform`.)

- [ ] **Step 4: Adicionar fallback usando `/proc/<pid>/cwd` no Unix**

Quando OSC 7 não está disponível, em cada poll da PTY tentar:
```rust
#[cfg(unix)]
fn read_cwd_from_proc(pid: u32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{}/cwd", pid)).ok()
        .or_else(|| {
            // macOS doesn't have /proc; use lsof or just rely on OSC 7
            None
        })
}
```

No macOS, sem `/proc`, dependemos de OSC 7 (zsh emite por default em macOS recentes). Documentar isso como limitação.

- [ ] **Step 5: Testar build**

```bash
cargo check
cargo test -p termai-core
```

- [ ] **Step 6: Smoke test**

```bash
cargo run --release
```
Dentro do termAI: `cd /tmp && pwd`. Pane.cwd deve atualizar (verificar via log temporário se possível).

Remover qualquer `dbg!`/`println!` antes de commitar.

- [ ] **Step 7: Commit**

```bash
git add crates/termai-app/src/pane.rs crates/termai-core/src/lib.rs
git commit -m "feat(pane): track cwd via OSC 7 escape sequence"
```

---

### Task 7: Componente TabBar — layout + renderização

**Files:**
- Create: `crates/termai-app/src/ui/tab_bar.rs`
- Modify: `crates/termai-app/src/ui/mod.rs`

- [ ] **Step 1: Escrever testes primeiro (TDD)**

Criar `crates/termai-app/src/ui/tab_bar.rs`:

```rust
//! Tab bar component — renders the strip at the top of the window.

use crate::theme::tokens;

/// A single tab's layout rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TabRect {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Compute the layout for N tabs given the strip's available width.
/// Returns one TabRect per tab. Reserves `traffic_lights_reserve` pixels on the left.
pub fn layout_tabs(
    tab_count: usize,
    strip_width: f32,
    strip_height: f32,
    traffic_lights_reserve: f32,
) -> Vec<TabRect> {
    if tab_count == 0 {
        return vec![];
    }
    let available = strip_width - traffic_lights_reserve - tokens::CONNECTION_INDICATOR_SIZE - tokens::CONNECTION_INDICATOR_RIGHT_PAD;
    let mut per_tab = available / tab_count as f32;
    per_tab = per_tab.clamp(tokens::TAB_MIN_WIDTH_ABSOLUTE, tokens::TAB_MAX_WIDTH);

    let mut out = Vec::with_capacity(tab_count);
    let mut x = traffic_lights_reserve;
    for i in 0..tab_count {
        out.push(TabRect {
            index: i,
            x,
            y: 0.0,
            w: per_tab,
            h: strip_height,
        });
        x += per_tab;
    }
    out
}

/// Hit test: return the tab index containing the given mouse position, if any.
pub fn hit_test(tabs: &[TabRect], px: f32, py: f32) -> Option<usize> {
    for tab in tabs {
        if px >= tab.x && px < tab.x + tab.w && py >= tab.y && py < tab.y + tab.h {
            return Some(tab.index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_tabs_split_evenly_with_reserve() {
        let tabs = layout_tabs(3, 1000.0, 36.0, 78.0);
        assert_eq!(tabs.len(), 3);
        // Available = 1000 - 78 - 8 (indicator) - 8 (pad) = 906; per_tab = 302, clamped to 240
        assert_eq!(tabs[0].w, tokens::TAB_MAX_WIDTH);
        assert_eq!(tabs[0].x, 78.0);
        assert_eq!(tabs[1].x, 78.0 + tokens::TAB_MAX_WIDTH);
    }

    #[test]
    fn many_tabs_shrink_to_min() {
        let tabs = layout_tabs(20, 400.0, 36.0, 0.0);
        let expected = (400.0 - 16.0) / 20.0;
        let expected = expected.clamp(tokens::TAB_MIN_WIDTH_ABSOLUTE, tokens::TAB_MAX_WIDTH);
        assert_eq!(tabs[0].w, expected);
    }

    #[test]
    fn hit_test_returns_correct_index() {
        let tabs = layout_tabs(3, 1000.0, 36.0, 78.0);
        // Click in middle of tab 1
        let mid_x = tabs[1].x + tabs[1].w / 2.0;
        assert_eq!(hit_test(&tabs, mid_x, 18.0), Some(1));
        // Click in traffic lights area
        assert_eq!(hit_test(&tabs, 40.0, 18.0), None);
        // Click below strip
        assert_eq!(hit_test(&tabs, mid_x, 100.0), None);
    }

    #[test]
    fn zero_tabs_returns_empty() {
        assert!(layout_tabs(0, 1000.0, 36.0, 78.0).is_empty());
    }
}
```

- [ ] **Step 2: Declarar módulo**

Edit `crates/termai-app/src/ui/mod.rs`:
```rust
pub mod path_shorten;
pub mod tab_bar;
```

- [ ] **Step 3: Rodar testes — devem passar**

```bash
cargo test -p termai-app tab_bar
```
Expected: 4 testes passam.

- [ ] **Step 4: Adicionar função render**

Adicionar ao `tab_bar.rs`:

```rust
use termai_renderer::{Renderer, Vertex};

pub struct TabBarRenderInput<'a> {
    pub tabs: &'a [TabRect],
    pub active_index: usize,
    pub hovered_index: Option<usize>,
    pub hover_progress: f32,        // 0.0..1.0, animation progress for hover bg interp
    pub titles: &'a [String],       // one per tab, same order as tabs
    pub strip_width: f32,
}

pub fn render_tab_bar(
    input: &TabBarRenderInput,
    renderer: &mut Renderer,
    main_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    // 1. Strip background.
    renderer.build_rect(
        0.0,
        0.0,
        input.strip_width,
        tokens::TAB_STRIP_HEIGHT,
        tokens::CHROME_BG,
        main_vertices,
    );

    // 2. Bottom border of the strip.
    renderer.build_rect(
        0.0,
        tokens::TAB_STRIP_HEIGHT,
        input.strip_width,
        tokens::TAB_STRIP_BORDER,
        tokens::CHROME_BORDER,
        main_vertices,
    );

    // 3. Each tab.
    for tab in input.tabs {
        let is_active = tab.index == input.active_index;
        let bg = if is_active {
            tokens::CHROME_BG_ACTIVE
        } else if input.hovered_index == Some(tab.index) {
            // interpolate from CHROME_BG to a slightly lighter shade.
            interpolate(tokens::CHROME_BG, [0x22 as f32/255.0, 0x22 as f32/255.0, 0x22 as f32/255.0, 1.0], input.hover_progress)
        } else {
            tokens::CHROME_BG
        };

        renderer.build_rect(tab.x, tab.y, tab.w, tab.h, bg, main_vertices);

        // Vertical separator on the right edge (except for last tab).
        if tab.index < input.tabs.len() - 1 {
            renderer.build_rect(
                tab.x + tab.w - tokens::TAB_STRIP_BORDER,
                tab.y + 6.0,
                tokens::TAB_STRIP_BORDER,
                tab.h - 12.0,
                tokens::CHROME_BORDER,
                main_vertices,
            );
        }

        // Title text (centered).
        if let Some(title) = input.titles.get(tab.index) {
            let (cw, ch) = renderer.chrome_cell_size();
            let text_w = title.chars().count() as f32 * cw;
            let text_x = tab.x + (tab.w - text_w) / 2.0;
            let text_y = tab.y + (tab.h - ch) / 2.0;
            let color = if is_active { tokens::TEXT_PRIMARY } else { tokens::TEXT_MUTED };
            renderer.build_text_run(title, text_x, text_y, color, chrome_vertices);
        }

        // Active tab accent line (bottom 2px).
        if is_active {
            renderer.build_rect(
                tab.x,
                tab.y + tab.h - tokens::TAB_ACTIVE_ACCENT_HEIGHT,
                tab.w,
                tokens::TAB_ACTIVE_ACCENT_HEIGHT,
                tokens::ACCENT,
                main_vertices,
            );
        }
    }
}

fn interpolate(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
```

- [ ] **Step 5: Rodar cargo check**

```bash
cargo check -p termai-app
```

- [ ] **Step 6: Commit**

```bash
git add crates/termai-app/src/ui/tab_bar.rs crates/termai-app/src/ui/mod.rs
git commit -m "feat(ui): add tab_bar component with layout and rendering"
```

---

### Task 8: Integrar nova TabBar no main loop

**Files:**
- Modify: `crates/termai-app/src/main.rs` (substituir `build_tab_bar_cells`, `tab_bar_pixel_height`, `handle_tab_bar_click`, render loop)

- [ ] **Step 1: Remover `build_tab_bar_cells`**

Deletar a função `build_tab_bar_cells()` em `crates/termai-app/src/main.rs:313-368` (intervalo exato — confirmar antes de deletar).

- [ ] **Step 2: Atualizar `tab_bar_pixel_height`**

Substituir `crates/termai-app/src/main.rs:150-160` por:
```rust
fn tab_bar_pixel_height(&self) -> f32 {
    if self.tab_bar.tab_count() <= 1 {
        return 0.0;
    }
    theme::tokens::TAB_STRIP_HEIGHT + theme::tokens::TAB_STRIP_BORDER
}
```

- [ ] **Step 3: Adicionar state de hover na App**

Em `App` struct (~linha 87 de `main.rs`), adicionar:
```rust
hovered_tab: Option<usize>,
hover_started: Instant,
```

E inicializar no `App::new()`:
```rust
hovered_tab: None,
hover_started: Instant::now(),
```

- [ ] **Step 4: Computar títulos de tabs a partir de cwd**

Adicionar método helper em `App`:
```rust
fn tab_titles(&self) -> Vec<String> {
    let home = dirs::home_dir();
    self.tab_bar.tabs.iter().map(|tab| {
        let cwd = find_pane_ref(&tab.root, tab.focused_pane_id)
            .and_then(|p| p.cwd.clone())
            .or_else(|| std::env::current_dir().ok());
        match cwd {
            Some(p) => ui::path_shorten::shorten(p, home.as_deref(), 20),
            None => tab.title.clone(),
        }
    }).collect()
}
```

Adicionar dependência `dirs = "5"` em `crates/termai-app/Cargo.toml` se não existir:
```bash
grep "^dirs" crates/termai-app/Cargo.toml || echo 'add: dirs = "5"'
```

- [ ] **Step 5: Atualizar render loop**

Em `crates/termai-app/src/main.rs:1311` (onde `let tab_bar_cells = self.build_tab_bar_cells();` está), substituir por:

```rust
let titles = self.tab_titles();
let tab_layout = if self.tab_bar.tab_count() > 1 {
    ui::tab_bar::layout_tabs(
        self.tab_bar.tab_count(),
        self.renderer.as_ref().map(|r| r.width() as f32).unwrap_or(0.0),
        theme::tokens::TAB_STRIP_HEIGHT,
        theme::tokens::TRAFFIC_LIGHTS_RESERVE,
    )
} else {
    vec![]
};
```

E onde `if !tab_bar_cells.is_empty() { renderer.build_vertices(...); }` (~linha 1354), substituir por:

```rust
let mut chrome_vertices: Vec<Vertex> = Vec::new();

if !tab_layout.is_empty() {
    let hover_progress = self.hovered_tab.map(|_| {
        let elapsed = self.hover_started.elapsed().as_millis() as f32;
        (elapsed / theme::tokens::HOVER_TRANSITION_MS as f32).min(1.0)
    }).unwrap_or(0.0);

    let strip_width = renderer.width() as f32;
    let input = ui::tab_bar::TabBarRenderInput {
        tabs: &tab_layout,
        active_index: self.tab_bar.active,
        hovered_index: self.hovered_tab,
        hover_progress,
        titles: &titles,
        strip_width,
    };
    ui::tab_bar::render_tab_bar(&input, renderer, &mut vertices, &mut chrome_vertices);
}
```

E mudar a chamada final para `submit_frame(&vertices, &chrome_vertices)`.

- [ ] **Step 6: Atualizar `handle_tab_bar_click`**

Substituir `crates/termai-app/src/main.rs:371-402` por:
```rust
fn handle_tab_bar_click(&mut self, px: f64, py: f64) -> bool {
    let tab_h = self.tab_bar_pixel_height();
    if tab_h == 0.0 {
        return false;
    }
    let sy = py as f32 * self.scale_factor;
    if sy >= tab_h {
        return false;
    }
    let strip_width = self.renderer.as_ref().map(|r| r.width() as f32).unwrap_or(0.0);
    let tab_layout = ui::tab_bar::layout_tabs(
        self.tab_bar.tab_count(),
        strip_width,
        theme::tokens::TAB_STRIP_HEIGHT,
        theme::tokens::TRAFFIC_LIGHTS_RESERVE,
    );
    let sx = px as f32 * self.scale_factor;
    if let Some(idx) = ui::tab_bar::hit_test(&tab_layout, sx, sy) {
        self.tab_bar.switch_to(idx);
        return true;
    }
    false
}
```

- [ ] **Step 7: Adicionar hover tracking no mouse move**

Em `crates/termai-app/src/main.rs:951` (perto de onde `hovered_url` é resetado), adicionar:
```rust
let new_hover = {
    let sx = pos.0 as f32 * self.scale_factor;
    let sy = pos.1 as f32 * self.scale_factor;
    let tab_h = self.tab_bar_pixel_height();
    if tab_h > 0.0 && sy < tab_h {
        let strip_width = self.renderer.as_ref().map(|r| r.width() as f32).unwrap_or(0.0);
        let tab_layout = ui::tab_bar::layout_tabs(
            self.tab_bar.tab_count(),
            strip_width,
            theme::tokens::TAB_STRIP_HEIGHT,
            theme::tokens::TRAFFIC_LIGHTS_RESERVE,
        );
        ui::tab_bar::hit_test(&tab_layout, sx, sy)
    } else {
        None
    }
};
if new_hover != self.hovered_tab {
    self.hovered_tab = new_hover;
    if new_hover.is_some() {
        self.hover_started = Instant::now();
    }
    self.window.as_ref().unwrap().request_redraw();
}
```

- [ ] **Step 8: cargo check + visual smoke**

```bash
cargo check -p termai-app
cargo run --release
```
Expected: abrir 2+ tabs (Cmd+T). Strip deve aparecer com novo visual: tabs equal-width, accent magenta na ativa, separadores verticais, hover state.

- [ ] **Step 9: Commit**

```bash
git add crates/termai-app/src/main.rs crates/termai-app/Cargo.toml
git commit -m "feat(ui): replace cell-based tab bar with shape-based renderer"
```

---

### Task 9: Connection indicator placeholder

**Files:**
- Create: `crates/termai-app/src/ui/connection_indicator.rs`
- Modify: `crates/termai-app/src/ui/mod.rs`, `crates/termai-app/src/main.rs`

- [ ] **Step 1: Criar componente**

`crates/termai-app/src/ui/connection_indicator.rs`:
```rust
//! Connection indicator — small dot on the right edge of the tab strip.

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Disconnected,
    Connected,
    Analyzing,
}

pub fn render(
    state: State,
    strip_width: f32,
    pulse_t: f32,  // 0.0..1.0, used when Analyzing
    renderer: &Renderer,
    vertices: &mut Vec<Vertex>,
) {
    let size = tokens::CONNECTION_INDICATOR_SIZE;
    let x = strip_width - size - tokens::CONNECTION_INDICATOR_RIGHT_PAD;
    let y = (tokens::TAB_STRIP_HEIGHT - size) / 2.0;

    match state {
        State::Connected => {
            renderer.build_rect(x, y, size, size, tokens::TEXT_DIM, vertices);
        }
        State::Disconnected => {
            renderer.build_rect_outline(x, y, size, size, 1.0, tokens::TEXT_DIM, vertices);
        }
        State::Analyzing => {
            let alpha = tokens::CURSOR_FADE_MIN + (1.0 - tokens::CURSOR_FADE_MIN) * pulse_t;
            renderer.build_rect(x, y, size, size, tokens::with_alpha(tokens::ACCENT, alpha), vertices);
        }
    }
}
```

- [ ] **Step 2: Declarar módulo**

Edit `crates/termai-app/src/ui/mod.rs`:
```rust
pub mod connection_indicator;
pub mod path_shorten;
pub mod tab_bar;
```

- [ ] **Step 3: Renderizar no main loop**

No render loop de `main.rs`, após chamar `render_tab_bar`, adicionar:
```rust
if self.tab_bar.tab_count() > 1 {
    // Always Disconnected for now; real state comes in Task 16.
    ui::connection_indicator::render(
        ui::connection_indicator::State::Disconnected,
        renderer.width() as f32,
        0.0,
        renderer,
        &mut vertices,
    );
}
```

- [ ] **Step 4: cargo check + visual**

```bash
cargo check -p termai-app
cargo run --release
```
Expected: bolinha vazia (outline) aparece no canto direito do strip.

- [ ] **Step 5: Commit**

```bash
git add crates/termai-app/src/ui/connection_indicator.rs crates/termai-app/src/ui/mod.rs crates/termai-app/src/main.rs
git commit -m "feat(ui): add connection indicator placeholder"
```

---

## Phase 3 — Content area

### Task 10: Cursor com fade no blink

**Files:**
- Modify: `crates/termai-app/src/main.rs` (onde o cursor é renderizado dentro do `build_pane_cells` ou função similar)

- [ ] **Step 1: Localizar renderização do cursor**

```bash
grep -n "cursor\|blink" crates/termai-app/src/main.rs | head -20
```

Encontrar onde o cursor cell é construído (provavelmente em `build_pane_cells` quando uma cell corresponde à posição do cursor).

- [ ] **Step 2: Computar fade opacity**

Adicionar método em `App`:
```rust
fn cursor_opacity(&self) -> f32 {
    let elapsed = self.cursor_blink_start.elapsed().as_millis();
    let phase = (elapsed % theme::tokens::CURSOR_BLINK_MS) as f32 / theme::tokens::CURSOR_BLINK_MS as f32;
    // sine fade between CURSOR_FADE_MIN and 1.0
    let s = (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    theme::tokens::CURSOR_FADE_MIN + (1.0 - theme::tokens::CURSOR_FADE_MIN) * s
}
```

- [ ] **Step 3: Aplicar opacity quando cursor é desenhado**

Na função que constrói cells do cursor, em vez de cor sólida da `theme.cursor`, multiplicar alpha:
```rust
let mut cur_color = self.theme.cursor;
cur_color[3] = self.cursor_opacity();
```

E quando a pane não está focada, usar outline em vez de fill (veja próxima task).

- [ ] **Step 4: Forçar redraw contínuo enquanto cursor está visível**

No `AboutToWait` ou equivalente, garantir que o redraw aconteça em ~30fps quando há cursor visível. Procurar:
```bash
grep -n "request_redraw\|AboutToWait\|MainEventsCleared" crates/termai-app/src/main.rs
```

Se já há um ticker, OK. Se não, adicionar um `control_flow.set_wait_timeout(Duration::from_millis(33))` no event loop.

- [ ] **Step 5: cargo check + visual**

```bash
cargo run --release
```
Expected: cursor pulsa suavemente (não on/off bruto).

- [ ] **Step 6: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(cursor): smooth fade blink instead of binary on/off"
```

---

### Task 11: Cursor outline quando pane não focado

**Files:**
- Modify: `crates/termai-app/src/main.rs`

- [ ] **Step 1: Identificar onde `is_focused` é passado pra renderização**

`build_pane_cells(pane, is_focused)` já recebe esse flag. Localizar onde o cursor é desenhado e o flag está disponível.

- [ ] **Step 2: Quando não focado, marcar a cell do cursor com bg = bg normal e fg = accent (vai virar outline)**

Como o renderer atual desenha cells como retângulos preenchidos, "outline" requer um pouco mais. Estratégia: emitir o cursor como uma chamada explícita após o `build_pane_cells`, fora do grid:

Em vez de modificar `build_pane_cells`, adicionar uma passada extra no render loop (~linha 1364):
```rust
// Cursor (drawn after pane content for proper z-order)
for (rect, _) in &pane_cells {
    let pane = match find_pane_ref(&tab.root, rect.id) {
        Some(p) => p,
        None => continue,
    };
    let is_focused = rect.id == focused_id;
    let (cw_px, ch_px) = renderer.cell_size();
    let cx = rect.x + pane.terminal.cursor_x as f32 * cw_px;
    let cy = rect.y + pane.terminal.cursor_y as f32 * ch_px;

    let mut color = self.theme.cursor;
    color[3] = if is_focused { self.cursor_opacity() } else { 1.0 };

    if is_focused {
        renderer.build_rect(cx, cy, cw_px, ch_px, color, &mut vertices);
    } else {
        renderer.build_rect_outline(cx, cy, cw_px, ch_px, 1.0, color, &mut vertices);
    }
}
```

Também remover qualquer renderização de cursor que estava embutida em `build_pane_cells` (vai duplicar senão).

- [ ] **Step 3: cargo check + visual**

```bash
cargo run --release
```
Cmd+D para dividir pane. Cursor da pane focada pulsa em block; pane sem foco mostra cursor como outline 1px sem blink.

- [ ] **Step 4: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(cursor): outline style when pane is unfocused"
```

---

### Task 12: Pane focus border 1px accent

**Files:**
- Modify: `crates/termai-app/src/main.rs`

- [ ] **Step 1: Adicionar border render após pane content**

No render loop após dividers (~linha 1386), adicionar:
```rust
// Focused pane border
if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
    renderer.build_rect_outline(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        theme::tokens::ACCENT,
        &mut vertices,
    );
}
```

- [ ] **Step 2: cargo check + visual**

```bash
cargo run --release
```
Cmd+D, então Cmd+] (ou equivalente) para alternar foco. Pane focada deve ter borda magenta 1px.

- [ ] **Step 3: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(pane): magenta 1px border on focused pane"
```

---

### Task 13: Seleção com alpha 25%

**Files:**
- Modify: `crates/termai-app/src/main.rs` (selection rendering)

- [ ] **Step 1: Localizar onde seleção é aplicada às cells**

```bash
grep -n "selection\|in_selection\|Selection" crates/termai-app/src/main.rs | head -20
```

Provavelmente em `build_pane_cells` há lógica que altera o bg de cells dentro do range da seleção.

- [ ] **Step 2: Mudar abordagem — desenhar overlay alpha por cima**

Em vez de mexer no bg das cells, manter cells originais e desenhar retângulos alpha sobre a área da seleção. No render loop, após desenhar pane content e antes do cursor:

```rust
if let Some(ref sel) = self.selection {
    let (sx, sy, ex, ey) = sel.normalized();
    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
        let (cw_px, ch_px) = renderer.cell_size();
        let sel_color = theme::tokens::with_alpha(theme::tokens::ACCENT, theme::tokens::SELECTION_ALPHA);

        if sy == ey {
            renderer.build_rect(
                rect.x + sx as f32 * cw_px,
                rect.y + sy as f32 * ch_px,
                (ex.saturating_sub(sx) + 1) as f32 * cw_px,
                ch_px,
                sel_color,
                &mut vertices,
            );
        } else {
            // First row: from sx to end of line
            let pane = find_pane_ref(&tab.root, focused_id);
            let cols = pane.map(|p| p.terminal.cols).unwrap_or(80);
            renderer.build_rect(
                rect.x + sx as f32 * cw_px,
                rect.y + sy as f32 * ch_px,
                (cols - sx) as f32 * cw_px,
                ch_px,
                sel_color,
                &mut vertices,
            );
            // Middle rows: full width
            for row in (sy + 1)..ey {
                renderer.build_rect(
                    rect.x,
                    rect.y + row as f32 * ch_px,
                    cols as f32 * cw_px,
                    ch_px,
                    sel_color,
                    &mut vertices,
                );
            }
            // Last row: from 0 to ex
            renderer.build_rect(
                rect.x,
                rect.y + ey as f32 * ch_px,
                (ex + 1) as f32 * cw_px,
                ch_px,
                sel_color,
                &mut vertices,
            );
        }
    }
}
```

- [ ] **Step 3: Remover lógica antiga de tingir bg da cell no `build_pane_cells`**

Procurar e remover qualquer trecho que sobrescreve `bg` da cell quando ela está em seleção.

- [ ] **Step 4: cargo check + visual**

```bash
cargo run --release
```
Click-drag para selecionar texto. Selection deve aparecer como overlay magenta translúcido, texto subjacente continua visível com suas cores normais.

- [ ] **Step 5: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(selection): translucent accent overlay preserving text color"
```

---

### Task 14: Ghost text com cor dim

**Files:**
- Modify: `crates/termai-app/src/main.rs:1340` (cor hardcoded do ghost text)

- [ ] **Step 1: Substituir cor hardcoded**

Edit `crates/termai-app/src/main.rs:1340`:

Antes:
```rust
fg: [0.5, 0.5, 0.5, 0.8],
```

Depois:
```rust
fg: theme::tokens::TEXT_DIM,
```

- [ ] **Step 2: cargo check + visual**

```bash
cargo run --release
```
Esperar uma sugestão de autocomplete; cor deve ser cinza um pouco mais escuro (#5a5a5a vs anterior #808080 alpha 80%).

- [ ] **Step 3: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(ghost-text): use design token TEXT_DIM color"
```

---

### Task 15: Padding da área de conteúdo

**Files:**
- Modify: `crates/termai-app/src/main.rs:174-184` (`content_area()`)

- [ ] **Step 1: Aplicar padding no content_area()**

Substituir `content_area()` em `crates/termai-app/src/main.rs:174-184`:

```rust
fn content_area(&self) -> (f32, f32, f32, f32) {
    if let Some(ref renderer) = self.renderer {
        let w = renderer.width() as f32;
        let h = renderer.height() as f32;
        let tab_h = self.tab_bar_pixel_height();
        let search_h = self.search_bar_pixel_height();
        let x = theme::tokens::CONTENT_PADDING_LEFT;
        let y = tab_h + theme::tokens::CONTENT_PADDING_TOP;
        let cw = w - x - theme::tokens::CONTENT_PADDING_RIGHT;
        let ch = h - y - search_h - theme::tokens::CONTENT_PADDING_BOTTOM;
        (x, y, cw, ch)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}
```

Atualizar `pixel_to_cell_in_pane` (~linha 186) se necessário para considerar o padding (o `rect.x/y` já incorporará o padding via `tab.layout(cx, cy, cw, ch)`).

- [ ] **Step 2: cargo check + visual**

```bash
cargo run --release
```
Texto não cola na borda esquerda nem no topo da tab bar. Tem respiração.

- [ ] **Step 3: Commit**

```bash
git add crates/termai-app/src/main.rs
git commit -m "feat(content): add padding around terminal content"
```

---

## Phase 4 — Overlays

### Task 16: Search bar flutuante no topo-direito

**Files:**
- Create: `crates/termai-app/src/ui/search_bar.rs`
- Modify: `crates/termai-app/src/ui/mod.rs`
- Modify: `crates/termai-app/src/main.rs` (remover `build_search_bar_cells`, integrar novo)

- [ ] **Step 1: Criar componente search_bar**

`crates/termai-app/src/ui/search_bar.rs`:
```rust
//! Floating search bar (top-right corner).

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

pub const SEARCH_BAR_WIDTH: f32 = 280.0;
pub const SEARCH_BAR_HEIGHT: f32 = 32.0;
pub const SEARCH_BAR_OFFSET_TOP: f32 = 8.0;
pub const SEARCH_BAR_OFFSET_RIGHT: f32 = 8.0;
pub const SEARCH_BAR_PADDING_X: f32 = 12.0;
pub const SEARCH_ICON: &str = "⌕";

pub struct SearchBarInput<'a> {
    pub query: &'a str,
    pub match_count: usize,
    pub current_match: usize,
    pub strip_width: f32,
    pub content_top: f32,
}

pub fn render(
    input: &SearchBarInput,
    renderer: &mut Renderer,
    main_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    let x = input.strip_width - SEARCH_BAR_WIDTH - SEARCH_BAR_OFFSET_RIGHT;
    let y = input.content_top + SEARCH_BAR_OFFSET_TOP;

    // Drop shadow (simple offset rect with alpha — no blur).
    renderer.build_rect(
        x + 2.0, y + 4.0, SEARCH_BAR_WIDTH, SEARCH_BAR_HEIGHT,
        [0.0, 0.0, 0.0, 0.4],
        main_vertices,
    );

    // Background.
    renderer.build_rect(x, y, SEARCH_BAR_WIDTH, SEARCH_BAR_HEIGHT, tokens::CHROME_BG_ACTIVE, main_vertices);

    // Border.
    renderer.build_rect_outline(x, y, SEARCH_BAR_WIDTH, SEARCH_BAR_HEIGHT, 1.0, tokens::CHROME_BORDER, main_vertices);

    let (cw, ch) = renderer.chrome_cell_size();
    let text_y = y + (SEARCH_BAR_HEIGHT - ch) / 2.0;

    // Icon.
    renderer.build_text_run(SEARCH_ICON, x + SEARCH_BAR_PADDING_X, text_y, tokens::TEXT_MUTED, chrome_vertices);

    // Query text (or placeholder).
    let query_x = x + SEARCH_BAR_PADDING_X + 2.0 * cw;
    if input.query.is_empty() {
        renderer.build_text_run("Buscar...", query_x, text_y, tokens::TEXT_DIM, chrome_vertices);
    } else {
        renderer.build_text_run(input.query, query_x, text_y, tokens::TEXT_PRIMARY, chrome_vertices);
    }

    // Counter on right.
    if input.match_count > 0 {
        let counter = format!("{}/{}", input.current_match + 1, input.match_count);
        let counter_w = counter.chars().count() as f32 * cw;
        renderer.build_text_run(
            &counter,
            x + SEARCH_BAR_WIDTH - SEARCH_BAR_PADDING_X - counter_w,
            text_y,
            tokens::TEXT_MUTED,
            chrome_vertices,
        );
    }
}
```

- [ ] **Step 2: Declarar módulo**

Edit `crates/termai-app/src/ui/mod.rs`:
```rust
pub mod connection_indicator;
pub mod path_shorten;
pub mod search_bar;
pub mod tab_bar;
```

- [ ] **Step 3: Substituir caller em main.rs**

Em `crates/termai-app/src/main.rs:1328` (`let search_bar_cells = self.build_search_bar_cells();`) e :1399 (where it's rendered), remover ambos.

Adicionar após render do conteúdo das panes:
```rust
if let Some(ref search) = self.search {
    let input = ui::search_bar::SearchBarInput {
        query: &search.query,
        match_count: search.matches.len(),
        current_match: search.current,
        strip_width: renderer.width() as f32,
        content_top: self.tab_bar_pixel_height(),
    };
    ui::search_bar::render(&input, renderer, &mut vertices, &mut chrome_vertices);
}
```

- [ ] **Step 4: Deletar `build_search_bar_cells` e `search_bar_pixel_height`**

Como a search bar agora é flutuante (não ocupa altura), `search_bar_pixel_height` sempre retorna 0:
```rust
fn search_bar_pixel_height(&self) -> f32 {
    0.0
}
```

(Mantém o método chamado pelo `content_area()`; só não reserva espaço.)

Deletar `build_search_bar_cells` em `crates/termai-app/src/main.rs:497-540` (ou intervalo equivalente — confirmar antes).

- [ ] **Step 5: Atualizar highlight dos matches no `build_pane_cells`**

Localizar onde matches são highlighted (provavelmente cor sólida na cell). Substituir cor sólida por shape de overlay alpha após o pane content, similar ao que fizemos com seleção:

```rust
// Search match highlights
if let Some(ref search) = self.search {
    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
        let (cw_px, ch_px) = renderer.cell_size();
        for (i, &(row, col)) in search.matches.iter().enumerate() {
            let alpha = if i == search.current {
                theme::tokens::SEARCH_CURRENT_MATCH_ALPHA
            } else {
                theme::tokens::SEARCH_MATCH_ALPHA
            };
            let color = theme::tokens::with_alpha(theme::tokens::ACCENT, alpha);
            let query_len = search.query.chars().count() as f32;
            renderer.build_rect(
                rect.x + col as f32 * cw_px,
                rect.y + row as f32 * ch_px,
                query_len * cw_px,
                ch_px,
                color,
                &mut vertices,
            );
        }
    }
}
```

E remover qualquer tingimento de bg dentro de `build_pane_cells` relacionado a search.

- [ ] **Step 6: cargo check + visual**

```bash
cargo run --release
```
Cmd+F. Bar flutuante aparece no topo-direito com sombra. Digitar busca destaca matches em magenta translúcido. Esc fecha.

- [ ] **Step 7: Commit**

```bash
git add crates/termai-app/src/ui/search_bar.rs crates/termai-app/src/ui/mod.rs crates/termai-app/src/main.rs
git commit -m "feat(search): floating search bar in top-right with translucent matches"
```

---

### Task 17: AI overlay refinado

**Files:**
- Create: `crates/termai-app/src/ui/ai_overlay.rs`
- Modify: `crates/termai-app/src/ui/mod.rs`
- Modify: `crates/termai-app/src/main.rs` (remover `build_ai_overlay_cells`)

- [ ] **Step 1: Criar componente**

`crates/termai-app/src/ui/ai_overlay.rs`:
```rust
//! AI suggestion overlay — appears at the bottom of the focused pane.

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Risk { Low, Medium, High }

pub struct ActionView<'a> {
    pub label: &'a str,
    pub risk: Risk,
}

pub struct AiOverlayInput<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub actions: &'a [ActionView<'a>],
    pub pane_rect: (f32, f32, f32, f32),  // x, y, w, h of focused pane
    pub fade_alpha: f32,  // 0.0..1.0
}

pub fn render(
    input: &AiOverlayInput,
    renderer: &mut Renderer,
    main_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    let (px, py, pw, ph) = input.pane_rect;
    let pad_y = 12.0;
    let pad_x = 16.0;
    let line_h = 18.0;
    let title_line_h = 22.0;
    let action_count = input.actions.len() as f32;
    let total_h = pad_y * 2.0 + title_line_h + line_h + action_count * line_h;

    let oy = py + ph - total_h;
    let ox = px;

    // Background.
    let mut bg = tokens::CHROME_BG;
    bg[3] = input.fade_alpha;
    renderer.build_rect(ox, oy, pw, total_h, bg, main_vertices);

    // Top border (accent, 2px).
    let mut accent = tokens::ACCENT;
    accent[3] = input.fade_alpha;
    renderer.build_rect(ox, oy, pw, 2.0, accent, main_vertices);

    let mut cursor_y = oy + pad_y;
    let mut primary = tokens::TEXT_PRIMARY;
    let mut muted = tokens::TEXT_MUTED;
    let mut dim = tokens::TEXT_DIM;
    primary[3] = input.fade_alpha;
    muted[3] = input.fade_alpha;
    dim[3] = input.fade_alpha;

    // Title.
    renderer.build_text_run(input.title, ox + pad_x, cursor_y, primary, chrome_vertices);
    cursor_y += title_line_h;

    // Description (truncated to fit one line — simple char limit).
    let (cw, _) = renderer.chrome_cell_size();
    let max_chars = ((pw - 2.0 * pad_x) / cw) as usize;
    let truncated: String = if input.description.chars().count() > max_chars {
        input.description.chars().take(max_chars.saturating_sub(1)).collect::<String>() + "…"
    } else {
        input.description.to_string()
    };
    renderer.build_text_run(&truncated, ox + pad_x, cursor_y, muted, chrome_vertices);
    cursor_y += line_h;

    // Actions.
    for (i, action) in input.actions.iter().enumerate() {
        let num = format!("[{}]", i + 1);
        let num_w = renderer.measure_chrome_text(&num);
        renderer.build_text_run(&num, ox + pad_x, cursor_y, accent, chrome_vertices);
        renderer.build_text_run(action.label, ox + pad_x + num_w + cw, cursor_y, primary, chrome_vertices);

        // Risk dot on the right.
        let dot_color = match action.risk {
            Risk::Low => [0x5a as f32 / 255.0, 0xf7 as f32 / 255.0, 0x8e as f32 / 255.0, input.fade_alpha],
            Risk::Medium => [0xf3 as f32 / 255.0, 0xf9 as f32 / 255.0, 0x9d as f32 / 255.0, input.fade_alpha],
            Risk::High => [0xff as f32 / 255.0, 0x5c as f32 / 255.0, 0x57 as f32 / 255.0, input.fade_alpha],
        };
        let dot_x = ox + pw - pad_x - 6.0;
        let dot_y = cursor_y + 4.0;
        renderer.build_rect(dot_x, dot_y, 6.0, 6.0, dot_color, main_vertices);

        if matches!(action.risk, Risk::High) {
            renderer.build_text_run("high", dot_x - 5.0 * cw, cursor_y, muted, chrome_vertices);
        }

        cursor_y += line_h;
    }
}
```

- [ ] **Step 2: Declarar módulo**

```rust
pub mod ai_overlay;
pub mod connection_indicator;
pub mod path_shorten;
pub mod search_bar;
pub mod tab_bar;
```

- [ ] **Step 3: Substituir caller em main.rs**

Em `crates/termai-app/src/main.rs:1329` (`let overlay_cells = self.build_ai_overlay_cells();`) e :1405-1412 (render), substituir por:

```rust
if let Some(ref suggestion) = self.ai_overlay {
    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
        // Fade alpha: 1.0 until last 200ms of 10s lifetime, then linear fade.
        let elapsed = suggestion.created.elapsed().as_millis();
        let total = 10_000u128;
        let fade_start = total - theme::tokens::OVERLAY_FADE_MS;
        let fade_alpha = if elapsed < fade_start {
            1.0
        } else {
            let remaining = total.saturating_sub(elapsed) as f32;
            (remaining / theme::tokens::OVERLAY_FADE_MS as f32).max(0.0)
        };

        let actions: Vec<ui::ai_overlay::ActionView> = suggestion.actions.iter().map(|a| {
            ui::ai_overlay::ActionView {
                label: &a.label,
                risk: match a.risk.as_str() {
                    "high" => ui::ai_overlay::Risk::High,
                    "medium" => ui::ai_overlay::Risk::Medium,
                    _ => ui::ai_overlay::Risk::Low,
                },
            }
        }).collect();

        let input = ui::ai_overlay::AiOverlayInput {
            title: &suggestion.title,
            description: &suggestion.description,
            actions: &actions,
            pane_rect: (rect.x, rect.y, rect.w, rect.h),
            fade_alpha,
        };
        ui::ai_overlay::render(&input, renderer, &mut vertices, &mut chrome_vertices);
    }
}
```

- [ ] **Step 4: Deletar `build_ai_overlay_cells`**

Localizar e remover `crates/termai-app/src/main.rs:602-...` (`fn build_ai_overlay_cells`). Confirmar intervalo antes.

- [ ] **Step 5: cargo check + visual**

```bash
cargo run --release
```
Provocar erro (`comando-inexistente`) para disparar AI overlay. Visual: borda superior magenta 2px, número da ação em magenta, risk dot colorido, fade out suave após 10s.

- [ ] **Step 6: Commit**

```bash
git add crates/termai-app/src/ui/ai_overlay.rs crates/termai-app/src/ui/mod.rs crates/termai-app/src/main.rs
git commit -m "feat(ai-overlay): redesigned overlay with accent border and risk dots"
```

---

### Task 18: Connection indicator com estado real

**Files:**
- Modify: `crates/termai-app/src/ai.rs` (expor `is_connected`, `is_analyzing`)
- Modify: `crates/termai-app/src/main.rs`

- [ ] **Step 1: Adicionar API ao AiClient**

Ler `crates/termai-app/src/ai.rs` e adicionar (se ainda não existir):
```rust
impl AiClient {
    pub fn is_connected(&self) -> bool {
        // Implementação depende da arquitetura atual.
        // Se há um Arc<AtomicBool>, retornar seu valor.
        // Caso contrário, retornar `self.socket.is_some()` ou equivalente.
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_analyzing(&self) -> bool {
        self.analyzing.load(std::sync::atomic::Ordering::Relaxed)
    }
}
```

Se os campos `connected`/`analyzing` não existem, adicioná-los como `Arc<AtomicBool>` e atualizá-los nos pontos relevantes do código (conectar/desconectar, antes/depois de send).

- [ ] **Step 2: Substituir hardcoded state no render loop**

Em `main.rs` onde chamamos `connection_indicator::render(..., State::Disconnected, ...)`, substituir por:
```rust
let state = match self.ai_client.as_ref() {
    Some(client) if client.is_analyzing() => ui::connection_indicator::State::Analyzing,
    Some(client) if client.is_connected() => ui::connection_indicator::State::Connected,
    _ => ui::connection_indicator::State::Disconnected,
};

let pulse_t = if matches!(state, ui::connection_indicator::State::Analyzing) {
    let elapsed = Instant::now().duration_since(self.cursor_blink_start).as_millis();
    let phase = (elapsed % theme::tokens::PULSE_PERIOD_MS) as f32 / theme::tokens::PULSE_PERIOD_MS as f32;
    (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5
} else {
    0.0
};

ui::connection_indicator::render(state, renderer.width() as f32, pulse_t, renderer, &mut vertices);
```

- [ ] **Step 3: cargo check + visual**

```bash
cargo run --release
```
Sem motor Go rodando: bolinha outline (disconnected). Iniciar motor Go separadamente, indicator vira cheio. Provocar análise, indicator pulsa em magenta.

- [ ] **Step 4: Commit**

```bash
git add crates/termai-app/src/ai.rs crates/termai-app/src/main.rs
git commit -m "feat(ai): connection indicator reflects real AI client state"
```

---

## Phase 5 — Polish

### Task 19: Cursor style configurável

**Files:**
- Modify: `crates/termai-app/src/config.rs`
- Modify: `crates/termai-app/src/main.rs`

- [ ] **Step 1: Adicionar campo ao Config**

Edit `crates/termai-app/src/config.rs`. Adicionar:
```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct CursorConfig {
    pub style: String,  // "block" | "bar" | "underline"
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self { style: "block".to_string() }
    }
}
```

E adicionar `cursor: CursorConfig` à struct `Config` principal.

- [ ] **Step 2: Aplicar no render do cursor**

Em main.rs onde o cursor é desenhado (Task 11), ramificar por estilo:
```rust
match self.config.cursor.style.as_str() {
    "bar" => {
        renderer.build_rect(cx, cy, 2.0, ch_px, color, &mut vertices);
    }
    "underline" => {
        renderer.build_rect(cx, cy + ch_px - 2.0, cw_px, 2.0, color, &mut vertices);
    }
    _ => {
        if is_focused {
            renderer.build_rect(cx, cy, cw_px, ch_px, color, &mut vertices);
        } else {
            renderer.build_rect_outline(cx, cy, cw_px, ch_px, 1.0, color, &mut vertices);
        }
    }
}
```

- [ ] **Step 3: Testar com config**

Criar `~/.config/termai/config.toml`:
```toml
[cursor]
style = "bar"
```
Rodar termAI, cursor vira bar 2px. Trocar para `"underline"`, vira sublinhado.

- [ ] **Step 4: Commit**

```bash
git add crates/termai-app/src/config.rs crates/termai-app/src/main.rs
git commit -m "feat(cursor): configurable style (block/bar/underline)"
```

---

### Task 20: QA smoke checklist + cleanup

**Files:**
- Não modifica código — só roda checklist e abre PRs de fix caso necessário.

- [ ] **Step 1: Rodar `cargo check` em todos os crates**

```bash
cargo check --workspace
```
Expected: zero warnings novos.

- [ ] **Step 2: Rodar todos os testes**

```bash
cargo test --workspace
```
Expected: tudo verde.

- [ ] **Step 3: Rodar `cargo clippy` (se configurado)**

```bash
cargo clippy --workspace -- -D warnings
```
Se houver lints, abrir mini-task pra corrigir antes de seguir.

- [ ] **Step 4: Smoke test funcional manual**

Roteiro:
- [ ] Abrir termAI: visual minimal, dark, tabs (1 tab → sem strip; abrir Cmd+T)
- [ ] 3+ tabs: strip aparece, equal-width, accent magenta na ativa, separadores
- [ ] Click em tab → switch funciona
- [ ] Hover em tab inativa: bg interpola
- [ ] `cd ~/code` → título da tab atualiza
- [ ] Cmd+D split: borda magenta na pane focada
- [ ] Cmd+] alterna foco: borda muda
- [ ] Cursor pulsa suave; outline quando pane sem foco
- [ ] Click-drag selecionar texto: overlay magenta translúcido
- [ ] Cmd+F: search bar flutuante top-right; digitar → matches em magenta
- [ ] Provocar erro (`comando-fake`): AI overlay com borda magenta superior
- [ ] Connection indicator no canto direito do strip reflete estado da engine

- [ ] **Step 5: Atualizar CLAUDE.md**

Edit `CLAUDE.md` — atualizar a seção "termai-renderer" para refletir que tem segundo atlas + UI helpers, e adicionar nota sobre `theme::tokens` como source-of-truth do design system.

- [ ] **Step 6: Commit final**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md to reflect new UI architecture"
```

---

## Self-review

Spec coverage:
- [x] Design tokens — Task 1
- [x] Pipeline UI (resolvido via segundo atlas + helpers existentes) — Task 2
- [x] Preset ANSI termAI Dark default — Task 3
- [x] Window chrome macOS (fullsize_content_view) — Task 4
- [x] Tab bar nova com layout/hit_test — Tasks 7-8
- [x] cwd tracking pra tab titles — Task 6
- [x] path_shorten utility — Task 5
- [x] Hover state com transição 120ms — Task 8
- [x] Connection indicator (placeholder + real) — Tasks 9, 18
- [x] Cursor fade + outline — Tasks 10-11
- [x] Pane focus border — Task 12
- [x] Seleção alpha — Task 13
- [x] Ghost text dim — Task 14
- [x] Padding de conteúdo — Task 15
- [x] Search bar flutuante — Task 16
- [x] AI overlay refinado — Task 17
- [x] Cursor style configurável — Task 19
- [x] QA smoke — Task 20

Riscos do spec (Step 8 polish):
- Anti-aliasing multi-monitor: **não coberto** explicitamente. Anotar como follow-up — não bloqueia o release dessa rodada.
- Throttle de hover: a animação de hover usa `request_redraw()` na transição de estado, não em cada frame — já é "throttled" naturalmente.

Out-of-scope (parking lot do spec): nada incluído no plano.

Placeholder scan: nenhum "TODO/TBD/implement later" no plano.

Type consistency: `TabRect`, `TabBarRenderInput`, `SearchBarInput`, `AiOverlayInput`, `ActionView`, `Risk`, `State` (connection_indicator) — todos definidos e usados consistentemente.
