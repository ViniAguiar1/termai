//! Text shaping for ligatures.
//!
//! In a monospaced terminal each grid cell holds exactly one `char`, so for
//! most text we can rasterize per-cell directly. But programming fonts ship
//! GSUB tables that turn pairs/triples like `==`, `!=`, `=>`, `->`, `>=`
//! into single ligature glyphs (with no `char` of their own).
//!
//! `apply_ligatures_to_row` runs rustybuzz over a single grid row, looks for
//! shaper outputs that collapse N input chars to fewer output glyphs, and
//! patches the corresponding cells with `glyph_id` and `suppress_glyph`. The
//! atlas can then rasterize the ligature glyph by id.

use rustybuzz::{Face, UnicodeBuffer};

use crate::RenderCell;

/// Shape one grid row's "code-like" runs and inject ligature glyphs into the
/// row's cells. Whitespace and style boundaries split runs.
pub fn apply_ligatures_to_row(row: &mut [RenderCell], font_bytes: &[u8]) {
    let face = match Face::from_slice(font_bytes, 0) {
        Some(f) => f,
        None => return,
    };

    let mut i = 0;
    while i < row.len() {
        // Find the end of a same-style, non-whitespace run.
        let style_bold = row[i].bold;
        let style_italic = row[i].italic;
        let mut end = i;
        while end < row.len()
            && !row[end].ch.is_whitespace()
            && row[end].bold == style_bold
            && row[end].italic == style_italic
            && !row[end].suppress_glyph
        {
            end += 1;
        }

        if end - i >= 2 {
            shape_run(&mut row[i..end], &face);
        }

        // Skip the whitespace / boundary cell, then move on.
        i = if end == i { i + 1 } else { end };
    }
}

fn shape_run(cells: &mut [RenderCell], face: &Face<'_>) {
    // Build the text and remember which byte offset each cell starts at, so
    // rustybuzz "cluster" values map back to a cell index.
    let mut text = String::with_capacity(cells.len() * 2);
    let mut cluster_to_cell: Vec<usize> = Vec::with_capacity(cells.len());
    for (idx, cell) in cells.iter().enumerate() {
        let start_byte = text.len();
        // Pad cluster_to_cell to cover every byte this char occupies.
        let mut buf = [0u8; 4];
        let s = cell.ch.encode_utf8(&mut buf);
        for _ in 0..s.len() {
            cluster_to_cell.push(idx);
        }
        let _ = start_byte;
        text.push(cell.ch);
    }

    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(&text);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(face, &[], buffer);

    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();

    // Walk the output. Group consecutive glyphs by their cluster: a single
    // output glyph whose cluster spans multiple input cells is a ligature.
    let mut g = 0;
    while g < infos.len() {
        let cluster = infos[g].cluster as usize;
        let start_cell = cluster_to_cell.get(cluster).copied().unwrap_or(0);

        // How many input cells does this glyph (or run of glyphs at this
        // cluster) consume? We look at the next glyph's cluster.
        let next_cluster = infos
            .get(g + 1)
            .map(|n| n.cluster as usize)
            .unwrap_or(text.len());
        let next_cell = cluster_to_cell
            .get(next_cluster)
            .copied()
            .unwrap_or(cells.len());

        let cells_consumed = next_cell.saturating_sub(start_cell).max(1);

        // Count how many output glyphs map to this same cluster — usually 1,
        // but could be more for marks/diacritics.
        let mut glyphs_in_cluster = 1;
        while g + glyphs_in_cluster < infos.len()
            && infos[g + glyphs_in_cluster].cluster as usize == cluster
        {
            glyphs_in_cluster += 1;
        }

        // Only inject when this is a real collapse (>1 cell folded into 1
        // glyph) AND the x_advance roughly matches `cells_consumed * cell_w`,
        // so non-ligature substitutions (e.g. small caps) don't mess up the
        // grid. We don't have cell_w here; trust the GSUB output for now.
        if cells_consumed >= 2 && glyphs_in_cluster == 1 {
            let glyph_id = infos[g].glyph_id as u16;
            if let Some(cell) = cells.get_mut(start_cell) {
                cell.glyph_id = Some(glyph_id);
            }
            for c in start_cell + 1..(start_cell + cells_consumed).min(cells.len()) {
                cells[c].suppress_glyph = true;
            }
        }

        g += glyphs_in_cluster;
        // Avoid infinite loops on degenerate input.
        let _ = positions;
    }
}
