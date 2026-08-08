# PHPantom — Blade

This document is the implementation plan for Laravel Blade template
support in PHPantom. For Eloquent model support see `laravel.md`.
For general architecture see `ARCHITECTURE.md`.

---

## Philosophy

- **No application booting.** Consistent with `laravel.md`. We
  never run PHP or boot a Laravel application.
- **Signatures over call-site scanning.** A template's variable types
  come from its declared contract: the Bladestan-compatible chain in
  `src/blade/signature.rs` — `@bladestan-signature` docblock, first
  docblock before template code, `@props`/`@aware`, Blade's own
  component scope, and the backing component class. The template
  declares what it expects; call sites are then
  *validated* against that contract (BL9), exactly as a function
  signature works. Inferring types *from* call sites inverts the contract and
  produces "true for one caller" types, so it is not the foundation —
  the shipped call-site inference fallback for unannotated projects
  is layered strictly below every declared source. Projects running
  Bladestan (a PHPStan extension for Blade template analysis) get the
  full contract model in both the editor and CI from the same
  annotations.
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

## Overview

Blade templates (`.blade.php`) mix HTML, Blade directives, component
tags (`<x-alert>`, `<livewire:counter>`), and embedded PHP. The
mago-syntax parser only understands pure PHP. The strategy:

1. Preprocess `.blade.php` files into valid PHP.
2. Feed the virtual PHP through the existing pipeline (parser,
   resolver, completion, definition).
3. Map LSP response positions back to the original Blade file via a
   source map.

---

## Phase 1: Blade-to-PHP Preprocessor

The core preprocessor is implemented in `src/blade/`. It transforms
Blade templates into virtual PHP line-by-line, with a source map for
coordinate translation. The LSP pipeline (`with_file_content`,
`update_ast`, `did_close`) transparently handles Blade files.

---

## Phase 2: Component Support

### BL1. Blade-aware code actions

Code actions are currently disabled for `.blade.php` files because
text edits target virtual PHP coordinates and actions like "Import
class" insert `use` statements at the top of the file rather than
inside a `@php` / `<?php` block. Re-enable code actions with:

- Range translation (virtual PHP → Blade) for all text edits.
- Blade-aware code generation (e.g. insert `use` inside `@php`).
- Filtering out actions that don't make sense in Blade context.

### BL3. `<x-component>` tag parsing in preprocessor

New file: `src/blade/components.rs`

The preprocessor detects `<x-name ...>` and `</x-name>` tags and
converts them to PHP.

#### BL3a. Opening tags

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

#### BL3b. Closing tags

`</x-name>` becomes a comment: `/* /x-name */`

#### BL3c. Named slots

`<x-slot:title>` → `$title = new \Illuminate\Support\HtmlString('');`
`</x-slot>` → comment

#### BL3d. Implicit component variables

When inside a component tag region (between opening and closing tags),
inject:

```php
/** @var \Illuminate\View\ComponentAttributeBag $attributes */
$attributes = new \Illuminate\View\ComponentAttributeBag([]);
/** @var \Illuminate\Support\HtmlString $slot */
$slot = new \Illuminate\Support\HtmlString('');
```

### 11. `<livewire:component>` tag parsing

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

### BL4. Component and view name completion

#### BL4a. `<x-` completion

When the user types `<x-` in a Blade file, offer completions from:

- the component index (class-based components, kebab-case names)
- Anonymous component templates: view names whose key
  starts with `"components."`, with the prefix stripped and dots
  preserved (e.g. `"components.forms.input"` → `"forms.input"`)

Detection: check if the characters before the cursor match
`<x-` (possibly with a partial name typed). This is a Blade-level
context check done before the normal PHP completion pipeline.

Items should use `CompletionItemKind::Module` or `::Class` depending
on whether they're anonymous or class-backed.

#### BL4b. `<livewire:` completion

Same pattern. When the user types `<livewire:`, offer completions
from the Livewire index.

#### BL4c. `@include('` and `@extends('` view name completion

When the cursor is inside the string argument to `@include`,
`@includeIf`, `@includeWhen`, `@includeUnless`, `@includeFirst`,
`@extends`, `@each`, or a `view()` function call, offer completions
from the view index (dot-notation view names).

Detection: look for `@include('`, `@extends('`, or `view('` before
the cursor and check that the cursor is inside the quotes. The
trigger characters `'` and `"` are already registered.

#### BL4d. Component attribute completion

When the cursor is inside a `<x-component ` tag (after the component
name, before `>` or `/>`), resolve the component class and offer its
constructor parameter names as kebab-case attribute completions.

Offer both plain and `:` prefixed variants:
- `message` (string literal)
- `:message` (PHP expression)

For Livewire components, offer the class's public property names as
attribute completions.

### 14. Tests

Create `tests/blade_components.rs`:

- `<x-alert>` resolves to `App\View\Components\Alert`
- `<x-forms.input>` resolves to `App\View\Components\Forms\Input`
- `<x-card>` resolves to index component
  `App\View\Components\Card\Card`
- `<livewire:counter>` resolves to `App\Livewire\Counter`
- Anonymous component detection
- `<x-dynamic-component>` does not crash
- Attribute parsing: string, expression, Alpine passthrough, bare,
  short syntax

Extend `tests/completion_blade.rs`:

- `<x-` triggers component name completions
- `<livewire:` triggers Livewire component name completions
- `@include('` triggers view name completions
- `<x-alert ` triggers attribute completions
- `$component->` after component instantiation
- `$attributes->` in component templates

---

## Phase 3: Cross-File View Intelligence

### BL5. Go-to-definition for view names and components

#### BL5a. View name go-to-definition

Inside `@include('users.index')`, `@extends('layouts.app')`, or
`view('welcome')`:

1. Extract the view name string at the cursor position.
2. Look up in the view index (`blade_view_path`).
3. Return a `Location` pointing to the resolved file.

#### BL5b. Component tag go-to-definition

On `<x-alert>`:

1. Extract the component name.
2. Look up in the component index to get the FQN.
3. Use `find_or_load_class` + `fqn_uri_index` to find the
   source file.
4. Return a `Location` pointing to the class definition.

On `<livewire:counter>`:

1. Same pattern using the Livewire index.

### BL6. Signature merging for `@extends`

When template A contains `@extends('layouts.app')`:

1. Resolve `layouts.app` via the view index to a file path.
2. Read or preprocess that file.
3. Extract `@var` declarations from its `@php` blocks.
4. Merge those declarations into template A's virtual PHP prologue,
   following the Bladestan covariance model:
   - Variables only in child: use child type.
   - Variables only in parent: use parent type.
   - Variables in both: child may narrow but not widen.
   - Walk the chain recursively if the parent also `@extends`.

This gives child templates access to the parent's declared
variables without the user redeclaring them.

### 18. Tests

Create `tests/definition_blade.rs`:

- Go-to-definition on `@include('users.index')` → view file
- Go-to-definition on `@extends('layouts.app')` → layout file
- Go-to-definition on `<x-alert>` → component class
- Go-to-definition on `<livewire:counter>` → Livewire class

Extend `tests/completion_blade.rs`:

- Variables from parent layout available in child via `@extends`
- Component class constructor types available in template

---

## Phase 4: Blade Directive Completion

### BL7. Directive name completion

When the user types `@` in a Blade file (outside `{{ }}`, `@php`
blocks, and string literals), offer completions for all known Blade
directives with snippet templates.

Each completion inserts a snippet with tab stops:

```
@if ($1)
    $0
@endif
```

```
@foreach ($1 as $2)
    $0
@endforeach
```

```
@include('$1')
```

```
@props([$1])
```

```
@inject('$1', '$2')
```

```
@php
$0
@endphp
```

Detection: The `@` trigger character is already registered. In
`handle_completion`, check `is_blade_file` and that the cursor is in
an HTML/directive context (not inside `{{ }}`, not inside a `@php`
block, not inside a string literal).

### 20. Tests

Extend `tests/completion_blade.rs`:

- `@` triggers directive name completions
- `@if` partial triggers filtered directive completions
- No directive completion inside `{{ }}` or `@php` blocks

---

## Phase 5: Template Contracts and Cross-File Flow

This phase aligns PHPantom's Blade understanding with Bladestan (a
PHPStan extension for statically analyzing Blade templates). The two
tools share one contract model: the same annotation gives autocomplete
and hover in the editor (PHPantom) and type checking in CI (Bladestan).
Where Bladestan defines a concept (signature chain, covariant merging,
call-site validation), we implement the same semantics so the
ecosystem converges on one way to type a template.

### BL10. Cross-file `@section` / `@stack` name intelligence

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

### BL11. Custom directive discovery

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
- directive name completion (BL7) includes them;
- registered component namespaces/paths extend the discovery index.

### BL9. `view()` call-site validation

The diagnostics counterpart of the declaration chain, matching Bladestan's
call-site validation rule: where a template has a
declared signature, a `view('name', [...])` call (and
`View::make()`, mailable content, `Route::view()` data,
`@include('name', [...])` inside templates) with a literal array
argument is checked against the merged signature as an array shape —
missing required variables, unknown extras, and type mismatches each
get a diagnostic. Templates without a signature produce no call-site
diagnostics. This gives the editor the same errors Bladestan reports
in CI, live while typing, from one annotation.

---

## Phase 6: Editor Tooling Parity

Other Blade-aware editors ship a full structure view, folding builder,
and formatter alongside their directive inspections.
`document_symbols.rs`, `folding.rs`, and `formatting.rs` have no Blade
awareness at all today, unlike `inlay_hints.rs` and `semantic_tokens.rs`,
which both check `is_blade_file` and translate positions through
`BladeSourceMap`. These items close that gap.

### BL13. Mismatched and unbalanced directive diagnostics

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

No dependency on component parsing (BL3) — this operates on the raw directive token stream and can land
independently.

#### Tests

New file `tests/diagnostics_blade.rs`:
- `@foreach(...) @endif` → mismatched-directive diagnostic
- `@if(...)` with no `@endif` before EOF → unclosed-directive diagnostic
- correctly paired directives → no diagnostic

### BL14. Folding ranges for Blade files

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

#### Tests

New file `tests/folding_blade.rs`:
- `@foreach`/`@endforeach` folds
- `<x-alert>`...`</x-alert>` folds
- fold ranges land on the correct Blade lines, not virtual-PHP lines

### BL15. Document outline (symbols) for Blade files

A `.blade.php` file today reports no outline, or an outline positioned
in virtual-PHP coordinates, because `document_symbols.rs` never
translates through the source map.

- Translate symbol ranges/selection ranges through
  `source_map.php_to_blade`.
- Build a Blade-native symbol tree on top of the translated PHP
  symbols: `@section`s and `@push`/`@stack` blocks as top-level
  symbols, `<x-component>` tags as child symbols showing the resolved
  component FQN once component parsing (BL3) lands — degrade to the bare tag name if the component
  doesn't resolve.
- Matches the structure-view behaviour other Blade-aware editors
  already provide.

#### Tests

New file `tests/document_symbols_blade.rs`:
- `@section('content')` appears as an outline entry
- `<x-alert>` appears as an outline entry with the resolved FQN once
  component parsing (BL3) is in place

### BL16. Blade-aware formatting

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

#### Feasibility research: proxying `blade-formatter` vs. a native basic formatter

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

#### Tests

- `formatting` on a `.blade.php` file returns no edits (short-term
  behaviour) rather than corrupting the file, until the long-term
  model lands.

### BL17. `format --check` CLI subcommand for CI

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

#### Tests

- `format --check` on an already-formatted project exits 0 with no
  output.
- `format --check` on a project with an unformatted `.blade.php` file
  exits non-zero and names the file.
- `format` (no `--check`) rewrites the file in place and a second run
  is a no-op.

---

## Implementation Sequence

Phase 1 is complete (steps 1-3): the preprocessor, LSP pipeline
integration, source mapping, `$loop`/`@session`/`@error`/`@context`
implicit variables, stub directives, verbatim regions, `languageId`
check, and code action suppression are all shipped.

The remaining steps build on the existing preprocessor:

### Step 4: Blade-aware code actions (BL1)

Translate every text edit back to Blade coordinates, generate
Blade-shaped code (a `use` statement belongs inside `@php`), and drop
the actions that make no sense in a template.

**Deliverable:** Code actions are re-enabled for `.blade.php` files.

### Step 5: Component tag parsing (BL3, items 11-12)

Implement `src/blade/components.rs`. Parse `<x-...>` and
`<livewire:...>` tags. Handle `@props`, `@aware`, named slots.

**Deliverable:** `$component->` after `<x-alert>` produces
completions from the Alert class. `$attributes->` works in component
templates.

### Step 6: Name completions (BL4)

Implement `<x-`, `<livewire:`, `@include('`, and component attribute
completions.

**Deliverable:** Typing `<x-` shows available components. Typing
`@include('` shows available views. Typing attributes inside
`<x-alert ` shows constructor parameter names.

### Step 7: Directive completion (BL7)

Implement `@` directive name completion with snippets.

**Deliverable:** Typing `@` in a Blade file shows all known
directives with snippet templates.

### Step 8: Cross-file intelligence (BL5, BL6)

Implement go-to-definition for view names and component tags.
Implement `@extends` signature merging.

**Deliverable:** Ctrl-click on `@include('users.index')` jumps to
the file. Parent layout variables are available in child templates.

### Step 9: Template contracts (BL9, BL10, BL11)

Call-site validation builds on the shipped declaration chain
(`src/blade/signature.rs`). Section/stack intelligence and custom
directive discovery are independent and can land in either order.

**Deliverable:** A template with a `@bladestan-signature` docblock
gets typed completion for its declared variables, and a `view()`
call missing a required variable gets a diagnostic.

### Step 10: Editor tooling parity (BL13-BL16)

Implement directive-pair validation (BL13), then Blade-position
translation for folding (BL14) and document symbols (BL15) — both
follow the same `php_to_blade` pattern already used by inlay hints
and semantic tokens. Formatting (BL16) starts with disabling the
feature safely for `.blade.php` and defers the full indentation
model.

**Deliverable:** `@foreach ... @endif` is flagged as a diagnostic.
Folding and the outline view work correctly on `.blade.php` files.
`textDocument/formatting` no longer risks corrupting a Blade file.

---

## Editor Integration Notes

### File extension detection

The server activates Blade preprocessing when:
- The URI ends with `.blade.php`, OR
- The `languageId` in `did_open` is `"blade"`.

### Zed extension

PHPantom's plain-PHP wiring has merged into Zed's official PHP
extension, so this repo no longer bundles its own `zed-extension/`.
That extension will not grow Blade support. The Blade language
registration (tree-sitter grammar, `.blade.php` association,
`languageId: "blade"` wiring) belongs to the separate `zed-laravel`
extension — see planned `zed-laravel` extension.

### Other editors

- **VS Code:** Extensions like Laravel Blade Snippets set
  `languageId` to `"blade"`. PHPantom's VS Code integration would
  need to register for both `"php"` and `"blade"` language IDs.
- **Neovim:** `lspconfig` can be configured to send `.blade.php`
  files to PHPantom with the correct `languageId`.
