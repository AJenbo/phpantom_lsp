use tower_lsp::lsp_types::Position;

/// Source map from virtual PHP back to original Blade positions.
#[derive(Debug, Clone)]
pub struct BladeSourceMap {
    /// Per-line column anchor points.
    ///
    /// Each entry is a pair `(blade_utf16_col, php_utf16_col)` representing
    /// a synchronisation point: position `blade_utf16_col` in the original
    /// Blade line corresponds to position `php_utf16_col` in the virtual
    /// PHP line.  Between two adjacent anchors the mapping is linear (1:1
    /// for PHP content, 0:N for boilerplate replacements).
    pub adjustments: Vec<Vec<(u32, u32)>>,
    /// Number of prologue lines the preprocessor injected before the
    /// first template line.  At least [`super::PROLOGUE_LINES`]; larger
    /// when call-site-inferred `@var` declarations are injected.
    pub prologue_lines: u32,
}

impl Default for BladeSourceMap {
    fn default() -> Self {
        Self {
            adjustments: Vec::new(),
            prologue_lines: super::PROLOGUE_LINES,
        }
    }
}

impl BladeSourceMap {
    pub fn blade_to_php(&self, pos: Position) -> Position {
        let line = pos.line as usize;
        let virtual_line = line as u32 + self.prologue_lines;

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

    /// Map a virtual-PHP position back to Blade, clamping prologue
    /// positions to the start of the template.
    ///
    /// Prefer [`Self::try_php_to_blade`] whenever the result becomes a text
    /// edit or a range the user is sent to: the clamp invents a position the
    /// template never had.
    pub fn php_to_blade(&self, pos: Position) -> Position {
        self.try_php_to_blade(pos).unwrap_or(Position {
            line: 0,
            character: 0,
        })
    }

    /// Map a virtual-PHP position back to Blade, or `None` when it falls in
    /// the preprocessor's prologue.
    ///
    /// The prologue holds declarations no template wrote (`$errors`,
    /// `$__env`, the injected `@var` docblocks, the `extends` clause of a
    /// synthesized `$this` wrapper class), so there is no template text
    /// behind it and no position to map to.
    pub fn try_php_to_blade(&self, pos: Position) -> Option<Position> {
        if pos.line < self.prologue_lines {
            return None;
        }
        let line = (pos.line - self.prologue_lines) as usize;

        if line >= self.adjustments.len() {
            return Some(Position {
                line: line as u32,
                character: pos.character,
            });
        }

        let line_adj = &self.adjustments[line];
        if line_adj.is_empty() {
            return Some(Position {
                line: line as u32,
                character: pos.character,
            });
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
                return Some(Position {
                    line: line as u32,
                    character: best_b,
                });
            }

            if max_b_offset == 0 {
                // PHP boilerplate mapped to a single Blade position.
                // EVERYTHING in this PHP segment maps to best_b.
                return Some(Position {
                    line: line as u32,
                    character: best_b,
                });
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

        Some(Position {
            line: line as u32,
            character: best_b + char_offset,
        })
    }
}
