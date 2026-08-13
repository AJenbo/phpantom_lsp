<?php

/**
 * PHP Showcase — Hover
 *
 * What hovering a symbol shows, including where the symbol came from.
 *
 * One of the demo files listed in README.md. Supporting fixtures live in
 * scaffolding/scaffolding.php (namespace Demo\Scaffolding), and the runtime
 * assertions that verify the type claims in the comments below live in
 * scaffolding/assertions.php.
 */

namespace Demo;

use Demo\Scaffolding;

// ── Hover: Origin Indicators ────────────────────────────────────────────────

class HoverOriginsDemo extends Scaffolding\Model implements Scaffolding\Renderable
{
    public function demo(): void
    {
        // Hover on `format` → "◆ implements Scaffolding\Renderable"
        $this->format('earth');

        // Hover on `toArray` → "↑ overrides Scaffolding\Model"
        $this->toArray();

        // Hover on `getName` → no indicator (inherited, not overridden)
        $this->getName();
    }

    // Implements Scaffolding\Renderable (Scaffolding\Model has no format method)
    public function format(string $template): string { return ''; }

    // Overrides the abstract toArray() from Scaffolding\Model
    public function toArray(): array { return []; }
}
