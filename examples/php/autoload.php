<?php

/**
 * Loads the whole Demo\ namespace tree. No Composer needed: this project
 * has no external dependencies, just its own files.
 *
 * Scaffolding comes first because every demo file depends on it. The demo
 * files themselves declare no cross-file parents, so their order is only
 * alphabetical.
 */

require_once __DIR__ . '/scaffolding/scaffolding.php';

require_once __DIR__ . '/code_actions.php';
require_once __DIR__ . '/code_lens.php';
require_once __DIR__ . '/completion.php';
require_once __DIR__ . '/definition.php';
require_once __DIR__ . '/diagnostics.php';
require_once __DIR__ . '/hover.php';
require_once __DIR__ . '/inlay_hints.php';
require_once __DIR__ . '/semantic_tokens.php';
require_once __DIR__ . '/signature_help.php';
