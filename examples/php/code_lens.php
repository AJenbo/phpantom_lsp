<?php

/**
 * PHP Showcase — Code Lens
 *
 * The annotations rendered above a declaration.
 *
 * One of the demo files listed in README.md. Supporting fixtures live in
 * scaffolding/scaffolding.php (namespace Demo\Scaffolding), and the runtime
 * assertions that verify the type claims in the comments below live in
 * scaffolding/assertions.php.
 */

namespace Demo;

use Demo\Scaffolding;

// ── Code Lens: prototype method annotations ─────────────────────────────────
// Open this class and look at the gutter above each method. PHPantom shows
// clickable annotations ("↑ ParentClass::method" or "◆ Interface::method")
// that navigate to the parent/interface declaration.
class CodeLensDemo extends Scaffolding\ScaffoldingAbstractShape implements Scaffolding\ScaffoldingDrawable
{
    // ↑ Scaffolding\ScaffoldingAbstractShape::area  — click to jump to abstract declaration
    public function area(): float { return 3.14; }

    // ↑ Scaffolding\ScaffoldingAbstractShape::perimeter
    protected function perimeter(): float { return 6.28; }

    // ◆ Scaffolding\ScaffoldingDrawable::draw  — interface implementations use ◆
    public function draw(string $color, float $opacity = 1.0): void {}
}
