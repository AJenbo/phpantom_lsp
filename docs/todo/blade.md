# PHPantom — Blade

Known gaps and planned features in PHPantom's Laravel Blade template
support. For Eloquent model support see `laravel.md`. For the shipped
preprocessor pipeline and module layout see `ARCHITECTURE.md`.

The strategy, implemented in `src/blade/`: preprocess `.blade.php`
files (recognised by URI suffix, or by a `did_open` `languageId` of
`"blade"`) into valid virtual PHP, feed the virtual PHP through the
existing pipeline (parser, resolver, completion, definition), and map
response positions back to the original Blade file through a source
map. Every item below builds on that pipeline.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier, with a dependent item placed after its
dependency.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## Out of scope (and why)

| Item | Reason |
|------|--------|
| Editor-side Blade language registration | The tree-sitter grammar, `.blade.php` file association, and `languageId: "blade"` wiring belong to editor extensions, not the server. Zed's official PHP extension (which absorbed PHPantom's plain-PHP wiring; this repo no longer bundles `zed-extension/`) will not grow Blade support — that registration belongs to the planned `zed-laravel` extension. On VS Code, Blade extensions already set `languageId` to `"blade"`, so PHPantom's integration needs to register for both `"php"` and `"blade"`. Neovim's `lspconfig` can be configured to send `.blade.php` files with the correct `languageId`. |
| Rendering or booting to resolve templates | Consistent with `laravel.md`: we never run PHP or boot a Laravel application. |

---

## Philosophy

- **No application booting.** Consistent with `laravel.md`. We
  never run PHP or boot a Laravel application.
- **Signatures over call-site scanning.** A template's variable types
  come from its declared contract: the Bladestan-compatible chain in
  `src/blade/signature.rs` — `@bladestan-signature` docblock, first
  docblock before template code, `@props`/`@aware`, Blade's own
  component scope, the backing component class, and the layouts the
  template `@extends`. The template declares what it expects; call
  sites are then *validated* against that contract
  (`src/blade/contract.rs` and `src/diagnostics/blade_call_site.rs`),
  exactly as a function signature works. Inferring types *from* call
  sites inverts the contract and produces "true for one caller" types,
  so it is not the foundation — the shipped call-site inference
  fallback for unannotated projects is layered strictly below every
  declared source. Projects running Bladestan (a PHPStan extension for
  Blade template analysis) get the full contract model in both the
  editor and CI from the same annotations.
- **Discovery is just directory walks.** Walking the configured view
  roots and the directories the component namespaces live in
  (`src/blade/discovery.rs`) is the full extent of external Blade file
  discovery. Paths are converted to view names and component names via
  string transforms. A namespace no PSR-4 mapping of the project's own
  `composer.json` covers (a vendor package that registered a component
  namespace) is read off the class index instead, since there is no
  directory to walk for it.
- **PSR-4 says where a namespace lives, not what a component is
  called.** A mapping resolves `App\View\Components` to the directory
  to walk, and once we know an FQN (e.g. `App\View\Components\Alert`)
  the existing `find_or_load_class` pipeline reads its source. The
  names themselves always come from the file paths.
- **Graceful degradation.** Unknown directives become comments. Failed
  component resolution produces comments. The user always gets partial
  completions rather than a broken file. The preprocessor must never
  produce invalid PHP.

---

## BL4. Component tag and attribute completion

**Impact: High · Effort: Medium**

### `<x-` completion

When the user types `<x-` in a Blade file, offer completions from:

- the component index (`blade_component_fqn` in
  `src/blade/discovery.rs`: class-based components, kebab-case names)
- Anonymous component templates: view names whose key
  starts with `"components."`, with the prefix stripped and dots
  preserved (e.g. `"components.forms.input"` → `"forms.input"`)

Detection: check if the characters before the cursor match
`<x-` (possibly with a partial name typed). This is a Blade-level
context check done before the normal PHP completion pipeline.

Items should use `CompletionItemKind::Module` or `::Class` depending
on whether they're anonymous or class-backed.

### `<livewire:` completion

Same pattern. When the user types `<livewire:`, offer completions
from the Livewire index (`livewire_component_fqn`).

### Component attribute completion

When the cursor is inside a `<x-component ` tag (after the component
name, before `>` or `/>`), resolve the component class and offer its
constructor parameter names as kebab-case attribute completions.

Offer both plain and `:` prefixed variants:
- `message` (string literal)
- `:message` (PHP expression)

For Livewire components, offer the class's public property names as
attribute completions.

### Tests

New file `tests/integration/completion_blade.rs`:

- `<x-` triggers component name completions
- `<livewire:` triggers Livewire component name completions
- `<x-alert ` triggers attribute completions
- `$component->` after component instantiation
- `$attributes->` in component templates

**Deliverable:** Typing `<x-` shows available components. Typing
attributes inside `<x-alert ` shows constructor parameter names.

---

## BL3. `<x-component>` tag parsing in preprocessor

**Impact: High · Effort: High**

The preprocessor already tracks HTML tag state so that a bound
attribute's expression (`:attr="$expr"`) survives into the virtual PHP
as a `blade_directive(...)` call, and `src/blade/component_tags.rs`
scans tags in the raw Blade source as call sites for signature
inference. What is missing is translating the tag itself into PHP: the
component class is never instantiated in the virtual PHP, so
`$component->` has nothing to resolve against.

### Opening tags

Parse `<x-component-name attr="val" :attr="$expr" ...>` or
`<x-component-name ... />` (self-closing).

1. Extract the component name (everything between `<x-` and the first
   whitespace or `>`/`/>`).
2. Look up the name in the component index (`blade_component_fqn`). If
   found, resolve the FQN.
3. Extract attributes:
   - `attr="literal"` → named arg with string value
   - `:attr="$expr"` → named arg with PHP expression value
   - `::attr="expr"` → ignored (Alpine.js passthrough)
   - Bare `attr` → named arg with `true`
   - `:$var` (short syntax) → named arg `var: $var`
4. Convert attribute names from kebab-case to camelCase for the
   constructor call.
5. Emit `$component = new \FQN(camelAttr: value, ...);`

If the component is not in the component index, check if it's an
anonymous component (a view under the `components.` prefix). For
anonymous components, emit a comment but still expose `$attributes`
and `$slot`.

For `<x-dynamic-component :component="$name" ...>`, emit
`echo $name;` so the expression gets parsed, but do not try to
resolve a target component.

### Closing tags

`</x-name>` becomes a comment: `/* /x-name */`

### Named slots

`<x-slot:title>` → `$title = new \Illuminate\View\ComponentSlot();`
`</x-slot>` → comment

### `<livewire:component>` tag parsing

Parse `<livewire:name :attr="$expr" ...>` or
`<livewire:name ... />`.

1. Extract the component name (everything between `<livewire:` and
   the first whitespace or `>`/`/>`).
2. Look up in the Livewire index (`livewire_component_fqn`). If found,
   resolve the FQN.
3. Extract attributes (same rules as `<x-...>`).
4. Emit `$component = new \FQN();` followed by property assignments
   for each attribute: `$component->attrName = $expr;`.

Livewire attribute names use camelCase on the class, so apply the
same kebab-to-camelCase conversion.

### Tests

New file `tests/integration/blade_components.rs`:

- `<x-alert>` resolves to `App\View\Components\Alert`
- `<x-forms.input>` resolves to `App\View\Components\Forms\Input`
- `<x-card>` resolves to index component
  `App\View\Components\Card\Card`
- `<livewire:counter>` resolves to `App\Livewire\Counter`
- Anonymous component detection
- `<x-dynamic-component>` does not crash
- Attribute parsing: string, expression, Alpine passthrough, bare,
  short syntax

**Deliverable:** `$component->` after `<x-alert>` produces
completions from the Alert class.

---

## BL13. Mismatched and unbalanced directive diagnostics

**Impact: Medium · Effort: Low-Medium**

`translate_directive` (`src/blade/directives.rs`) maps each directive
to PHP independently by name, with no check that a closing directive
matches the block it opened. `@foreach ($items as $item) ... @endif`
currently translates silently instead of producing a diagnostic.

- Track a stack of open control directives (`if`, `foreach`, `for`,
  `while`, `switch`, `unless`, `isset`, `empty`, `once`, `verbatim`,
  `fragment`, component/slot tags) during preprocessing, alongside the
  existing source map.
- When a closing directive doesn't match the top of the stack (or the
  stack is empty), emit a diagnostic at the closing directive's Blade
  position: "Expected `@endX`, found `@endY`" or "Unexpected `@endY`
  with no matching `@y`".
- Flag unclosed directives at end-of-file (stack non-empty when the
  file ends).
- Matches the mismatched-closing-directive inspection other Blade-aware
  editors already ship.

No dependency on component parsing (BL3) — this operates on the raw
directive token stream and can land independently.

### Tests

New file `tests/integration/diagnostics_blade.rs`:

- `@foreach(...) @endif` → mismatched-directive diagnostic
- `@if(...)` with no `@endif` before EOF → unclosed-directive diagnostic
- correctly paired directives → no diagnostic

---

## BL11. Custom directive discovery

**Impact: Medium · Effort: Low-Medium**

`Blade::directive('datetime', …)`, `Blade::if('env', …)`, and
component namespace registrations (`Blade::componentNamespace()`,
`Blade::anonymousComponentPath()`) in app and package service
providers declare project-specific directives. Scan literal
registrations — the same provider-scanning shape as the macro
scanner — so that:

- known custom directives stop degrading to comments in the
  preprocessor and instead map to expression-preserving PHP (their
  argument is still type-checked);
- `Blade::if('admin')` synthesizes the full family (`@admin`,
  `@elseadmin`, `@endadmin`, `@unlessadmin`);
- directive name completion (`DIRECTIVE_COMPLETIONS` in
  `src/blade/directives.rs`) includes them;
- registered component namespaces/paths extend the discovery index.

---

## BL1. Blade-aware code actions

**Impact: Medium · Effort: Medium**

Code actions are currently disabled for `.blade.php` files because
text edits target virtual PHP coordinates and actions like "Import
class" insert `use` statements at the top of the file rather than
inside a `@php` / `<?php` block. Re-enable code actions with:

- Range translation (virtual PHP → Blade) for all text edits.
- Blade-aware code generation (e.g. insert `use` inside `@php`).
- Filtering out actions that don't make sense in Blade context.

**Deliverable:** Code actions are re-enabled for `.blade.php` files.

---

## BL5. Go-to-definition for component tags

**Impact: Medium · Effort: Medium**

### Component tag go-to-definition

On `<x-alert>`:

1. Extract the component name.
2. Look up in the component index to get the FQN.
3. Use `find_or_load_class` + `fqn_uri_index` to find the
   source file.
4. Return a `Location` pointing to the class definition.

On `<livewire:counter>`: same pattern using the Livewire index.

### Tests

New file `tests/integration/definition_blade.rs`:

- Go-to-definition on `<x-alert>` → component class
- Go-to-definition on `<livewire:counter>` → Livewire class

**Deliverable:** Ctrl-click on `<x-alert>` jumps to the component
class.

---

## BL10. Cross-file `@section` / `@stack` name intelligence

**Impact: Medium · Effort: Medium**

`@section`/`@hasSection`/`@sectionMissing`/`@yield` and
`@push`/`@prepend`/`@stack` name arguments are cross-file string
keys: yields and stacks are declared in layouts, filled in children.
Index section and stack names per template (alongside the discovery
index in `src/blade/discovery.rs`, recording the `@extends` target),
then provide:

- completion of section/stack names inside child templates from the
  resolved layout chain, and vice versa in layouts from known
  children;
- go-to-definition between `@section('x')` and its `@yield('x')`;
- an unknown-section diagnostic when a child fills a section its
  layout chain never yields (dynamic names skip, as always).

---

## BL22. A template never learns anything from the templates that render it

**Impact: Medium · Effort: Medium**

`compute_blade_injected_vars` skips every Blade file in the caller
snapshot (`if self.is_blade_file(file_uri) { continue; }`, and
`view_caller_snapshot` filters them out again), so call-site inference
only ever reads controllers. A partial rendered exclusively from other
templates therefore gets nothing: neither the entries of an
`@include('partials.row', ['row' => $row])`, nor the item and key an
`@each` binds, reach it, and an unannotated partial has no type for the
variable it exists to display. Only the diagnostics path, which runs on
the rendering template itself, reads those sites today.

The comment gives the two reasons: a template must not feed itself, and
other templates' `@include`s would recurse. Both are addressable the way
the component-tag path already does it — that one *does* read Blade
callers, keying off `blade_virtual_content` rather than the raw source
(the symbol map's offsets are virtual-PHP offsets, so the raw Blade
source is the wrong thing to parse) and skipping the template's own URI.
What is left to settle is mutual rendering: A includes B includes A
resolves each against the other's cached scope, so the refresh pass needs
to converge rather than oscillate.

Until then a partial rendered only from templates must declare its own
`@bladestan-signature` to get completion and hover, which is what the
contract checks want anyway.

---

## BL14. Folding ranges for Blade files

**Impact: Low-Medium · Effort: Low**

`textDocument/foldingRange` on a `.blade.php` file currently returns
ranges in virtual-PHP coordinates, which don't line up with the
original template, because `folding.rs` never translates through the
source map.

- Translate each `FoldingRange` through `source_map.php_to_blade`
  before returning, matching the pattern in `inlay_hints.rs`.
- Add Blade-native fold regions the underlying PHP has no concept of:
  `@if`/`@endif` and friends (using the stack from BL13),
  `<x-component>`...`</x-component>` tag bodies, `@section`/
  `@endsection`, `@push`/`@endpush`.
- Matches the folding behaviour other Blade-aware editors already
  provide.

### Tests

New file `tests/integration/folding_blade.rs`:

- `@foreach`/`@endforeach` folds
- `<x-alert>`...`</x-alert>` folds
- fold ranges land on the correct Blade lines, not virtual-PHP lines

---

## BL15. Document outline (symbols) for Blade files

**Impact: Low-Medium · Effort: Medium**

A `.blade.php` file today reports no outline, or an outline positioned
in virtual-PHP coordinates, because `document_symbols.rs` never
translates through the source map.

- Translate symbol ranges/selection ranges through
  `source_map.php_to_blade`.
- Build a Blade-native symbol tree on top of the translated PHP
  symbols: `@section`s and `@push`/`@stack` blocks as top-level
  symbols, `<x-component>` tags as child symbols showing the resolved
  component FQN once component parsing (BL3) lands — degrade to the
  bare tag name if the component doesn't resolve.
- Matches the structure-view behaviour other Blade-aware editors
  already provide.

### Tests

New file `tests/integration/document_symbols_blade.rs`:

- `@section('content')` appears as an outline entry
- `<x-alert>` appears as an outline entry with the resolved FQN once
  component parsing (BL3) is in place

---

## BL16. Blade-aware formatting

**Impact: Low-Medium · Effort: High**

`formatting.rs` has no Blade awareness. `mago`'s formatter runs against
the virtual PHP buffer generated for `.blade.php` files, and its output
has no fixed relationship to the original directive/HTML structure —
there is no path today that safely reformats the original Blade
markup.

- Short term: explicitly disable `textDocument/formatting` for
  `.blade.php` (return no edits) rather than risk corrupting the file
  with virtual-PHP-shaped edits translated through the source map.
- Medium term: extend `formatting.rs`'s existing external-tool
  resolution (currently php-cs-fixer/Pint/phpcbf via Composer
  `require-dev`) to also detect a project-installed `blade-formatter`
  (npm, via `package.json`/`node_modules/.bin`) and proxy it over
  `--stdin`, matching how Pint is already invoked. See the feasibility
  research below for why this is a good fit despite `blade-formatter`
  not being a Composer tool.
- Long term: a Blade-native indentation model (directive nesting depth
  + HTML tag depth + embedded `@php`/`{{ }}` PHP formatting via
  `mago`) as the built-in fallback for projects without
  `blade-formatter` installed. This is the highest-effort item in the
  Blade backlog; most Blade projects already reach for a dedicated
  Blade formatter, which is why the external-tool path above should
  land first.

### Feasibility research: proxying `blade-formatter` vs. a native basic formatter

Investigated `blade-formatter` (the `shufo/blade-formatter` npm
package) as a possible thing to shell out to.

**Proxying it fits the existing external-tool pattern.**
`formatting.rs` already resolves php-cs-fixer/Pint/phpcbf by detecting
them in the project (`composer.json` `require-dev`, resolved via
Composer's bin-dir) and falls back to the built-in mago-formatter when
absent — the same "use the project's own tool if it's there, otherwise
built-in" shape the user wants here. `blade-formatter` is a reasonable
addition to that resolution chain, not something to reject outright.
The one wrinkle: it is an npm package, not a Composer one, so detection
needs its own path parallel to (not reusing) the `vendor/bin` resolver
— check `package.json` `devDependencies`/`dependencies` for
`blade-formatter` and resolve the binary via `node_modules/.bin/
blade-formatter`, with the same `.phpantom.toml` override/disable
shape (`blade-formatter = "..."` / `blade-formatter = ""`) as the
existing PHP tools. This also means Node must be present on the
machine for the external path to trigger at all; when it isn't, or the
package isn't installed, fall back to the built-in formatter exactly
like the PHP tools do today.

The two operational concerns from the initial pass (no result cache;
`--write` deletes-then-rewrites the target file) turn out not to be
blockers once invoked the way we already invoke Pint: it supports
`--stdin` (format code from stdin, formatted result on stdout), so we
would never let `blade-formatter` touch the file directly — we own the
write, the same way we already do for Pint via `--stdin-filename`. And
since we'd invoke it once per format request (never in a long-lived
watch mode), the lack of an internal cache is no different from how
php-cs-fixer/phpcs are already invoked fresh per request; there is no
extra cost specific to this tool. Net: prefer the project's own
`blade-formatter` install when present (best fidelity with what the
team already uses and reviews), built-in native formatter as the
fallback and long-term goal — matching the existing PHP formatter
precedent.

**How it actually formats, and what that implies for a native
implementation.** It is not AST-based. The pipeline
(`formatContentPipeline.ts`) runs ~30 regex-based string processors in
a pre-process/post-process sandwich around two off-the-shelf
formatters: `js-beautify` for the HTML "shell" and
`@prettier/plugin-php` for isolated PHP/Blade-brace expressions. Content
that would confuse those formatters (raw `@php` blocks, `<script>`,
`<style>`, comments, Alpine.js `x-data`/`x-init` attributes, component
props) is regex-extracted to placeholder tokens before beautifying and
spliced back in afterward. Directive-nesting indent is a *separate*
pass (`formatter.ts`'s `processTokenizeResult`/`processKeyword`):
it tokenizes each line with a bundled Blade TextMate grammar
(`syntaxes/blade.tmLanguage.json`, run through `vscode-textmate` +
`vscode-oniguruma`/WASM) purely to classify which tokens are Blade
keywords, then walks a hardcoded stack of directive-start/-end/-else
token lists (`indent.ts`) to raise/lower indent level per line, with
special-cased exceptions (`@case` inside `@switch` dedents one extra
level, `@break` inside `@if` doesn't indent, `@section`/`@push`/`@slot`
are self-closing when given a second argument, `@hasSection` is
"unbalanced" and never closes). It is a large surface of hand-tuned
edge cases, not a formal grammar.

The directive-nesting indent pass is the one part of this design that
translates cleanly to PHPantom, and we are better positioned to do it
than blade-formatter was: `src/blade/directives.rs` already has the
full directive table blade-formatter hardcodes in `indent.ts`, and
unlike blade-formatter (which had to pull in a TextMate grammar +
oniguruma WASM just to find directive tokens on a line), our
preprocessor already tokenizes Blade source precisely. What's missing
is (a) classifying each directive as indent-start / indent-end / else
(mechanical, from the existing table) and (b) an HTML tag-depth
counter interleaved with directive depth on the same line (open/close/
void/self-closing elements) — new code, but a single self-contained
pass, not a full HTML parser. Embedded `@php`/`{{ }}` expression
formatting can reuse `mago`'s formatter on isolated snippets the same
way diagnostics already isolate virtual-PHP buffers, rather than
needing a second PHP formatter dependency like blade-formatter does.

**Recommended scope for the native fallback formatter:** directive-
nesting indent + HTML tag indent only, reindenting existing lines
without rewriting their content (no attribute sorting, no line-wrap/
wrapping of long tags, no quote-style normalization, no Tailwind class
sorting). That covers the visible majority of blade-formatter's example
output (consistent indentation) while skipping almost all of its
~30-processor edge-case surface, which exists to handle content
rewriting we are choosing not to do. This is still a real chunk of work
(a new directive-classification table + an HTML depth scanner + the
`mago`-snippet-reformat glue), but is meaningfully smaller than full
parity with `blade-formatter`, does not require adding a
TextMate/oniguruma dependency the way blade-formatter's own approach
does, and is worth having on its own merits: it is the only
option for projects that don't have `blade-formatter` installed, and
if it ends up faster and at least as correct as `blade-formatter` (a
real possibility, given we skip the regex/placeholder round-tripping
entirely), it can become the default rather than staying a fallback.
See BL17 below for exposing it as a standalone CI check once it
reaches that bar.

### Tests

- `formatting` on a `.blade.php` file returns no edits (short-term
  behaviour) rather than corrupting the file, until the long-term
  model lands.

---

## BL17. `format --check` CLI subcommand for CI

**Impact: Low-Medium · Effort: Low-Medium** (depends on BL16)

Filed while researching BL16: once the native Blade formatter (and,
more generally, PHPantom's resolved formatting strategy — external
tool or built-in) is trustworthy enough to enforce, projects want a
non-editor way to verify a PR ran it, the same role `blade-formatter
-c`/`--check-formatted` plays today. `main.rs` currently only exposes
`analyse` and `fix` as CLI subcommands; there is no way to invoke
`textDocument/formatting`'s resolution logic (`formatting.rs`) outside
the LSP connection at all.

- Add a `format` subcommand (`phpantom_lsp format --project-root
  <DIR> [--check]`) that walks project PHP/Blade files, runs the same
  `resolve_strategy` external-tool-or-built-in logic `formatting.rs`
  already uses, and either writes the formatted result back or (with
  `--check`) exits non-zero and lists files that would change, without
  writing them — mirroring `blade-formatter -c -d` and `phpcs
  --dry-run`/`php-cs-fixer --dry-run` conventions projects already use
  in CI.
- This depends on the native Blade formatter model (BL16 long term)
  existing and being fast/correct enough that a maintainer would want
  it enforced in CI; do not build the CLI surface before that bar is
  met, since the CLI is only useful once there's a trustworthy
  formatter behind it (or a detected `blade-formatter`/`php-cs-fixer`
  external tool, which `--check` should honour identically to
  `format` without `--check`).

### Tests

- `format --check` on an already-formatted project exits 0 with no
  output.
- `format --check` on a project with an unformatted `.blade.php` file
  exits non-zero and names the file.
- `format` (no `--check`) rewrites the file in place and a second run
  is a no-op.
