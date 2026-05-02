use tower_lsp::lsp_types::Position;

/// Source map from virtual PHP back to original Blade positions.
#[derive(Debug, Clone, Default)]
pub struct BladeSourceMap {
    /// Per-line column anchor points.
    /// Each entry is (blade_col, php_col).
    pub adjustments: Vec<Vec<(u32, u32)>>,
}

impl BladeSourceMap {
    pub fn blade_to_php(&self, pos: Position) -> Position {
        let line = pos.line as usize;
        // Prologue adds 5 lines. Blade line 0 becomes virtual line 5.
        let virtual_line = (line + 5) as u32;

        if line >= self.adjustments.len() {
            return Position {
                line: virtual_line,
                character: pos.character,
            };
        }

        let line_adj = &self.adjustments[line];
        if line_adj.is_empty() {
            return Position {
                line: virtual_line,
                character: pos.character,
            };
        }

        let mut best_b = 0;
        let mut best_p = 0;

        for (b, p) in line_adj.iter() {
            if *b <= pos.character {
                best_b = *b;
                best_p = *p;
            } else {
                break;
            }
        }

        let char_offset = pos.character.saturating_sub(best_b);

        Position {
            line: virtual_line,
            character: best_p + char_offset,
        }
    }

    pub fn php_to_blade(&self, pos: Position) -> Position {
        if pos.line < 5 {
            return Position {
                line: 0,
                character: 0,
            };
        }
        let line = (pos.line - 5) as usize;

        if line >= self.adjustments.len() {
            return Position {
                line: line as u32,
                character: pos.character,
            };
        }

        let line_adj = &self.adjustments[line];
        if line_adj.is_empty() {
            return Position {
                line: line as u32,
                character: pos.character,
            };
        }

        let mut best_idx = 0;
        let mut best_b = 0;
        let mut best_p = 0;

        for (i, (b, p)) in line_adj.iter().enumerate() {
            if *p <= pos.character {
                best_idx = i;
                best_b = *b;
                best_p = *p;
            } else {
                break;
            }
        }

        let mut char_offset = pos.character.saturating_sub(best_p);

        if let Some((next_b, next_p)) = line_adj.get(best_idx + 1) {
            let max_b_offset = next_b.saturating_sub(best_b);
            let max_p_offset = next_p.saturating_sub(best_p);

            if max_p_offset == 0 {
                // PHP boilerplate mapped to zero-width Blade point?
                // This shouldn't happen with our anchor strategy, but be safe.
                return Position {
                    line: line as u32,
                    character: best_b,
                };
            }

            if max_b_offset == 0 {
                // PHP boilerplate mapped to a single Blade position.
                // EVERYTHING in this PHP segment maps to best_b.
                return Position {
                    line: line as u32,
                    character: best_b,
                };
            }

            // Normal 1:1 or N:M mapping.
            // If the ratios are different (e.g. multi-byte characters),
            // we could scale char_offset, but for PHPantom we mostly
            // deal with 1:1 code or 0:N boilerplate.
            // We'll stick to 1:1 interpolation but cap it to next_b.
            if char_offset > max_b_offset {
                char_offset = max_b_offset;
            }
        }

        Position {
            line: line as u32,
            character: best_b + char_offset,
        }
    }
}
