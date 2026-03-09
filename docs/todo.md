# PHPantom — Roadmap

This is the master index for all planned work. Each row links to the
domain document that contains the full specification. Items are
sequenced by **sprint priority** — what to build next to widen the type
intelligence lead, then close the LSP feature gap, maximising coverage
with each step.

| Label | Scale |
|---|---|
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low** |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

## Domain Documents

| Document | Scope |
|---|---|
| [Type Inference](todo/type-inference.md) | Generic resolution, conditional return types, type narrowing, stub attribute handling |
| [Completion](todo/completion.md) | Completion-specific improvements (enum return types, array shapes, expected values) |
| [Diagnostics](todo/diagnostics.md) | `@deprecated` warnings, resolution-failure diagnostics, unused `use` dimming, suppression intelligence |
| [Code Actions](todo/actions.md) | Import class, remove unused imports, implement missing methods, null coalescing simplification, extract function, inline variable, extract variable, inline function/method, switch→match |
| [LSP Features](todo/lsp-features.md) | Find references, document highlighting, document/workspace symbols, rename, code lens, inlay hints, PHPDoc generation, partial result streaming |
| [Signature Help](todo/signature-help.md) | Parameter descriptions, signature-level docs, default values, attribute/closure support |
| [Laravel](todo/laravel.md) | Model property gaps, relationship methods, type narrowing, custom builders |
| [Blade](todo/blade.md) | Preprocessor, component support, cross-file view intelligence |
| [Bug Fixes](todo/bugs.md) | Incorrect behaviour that should be fixed regardless of feature priority |
| [Configuration](todo/config.md) | Per-project `.phpantom.toml` file, PHP version override, diagnostic tool toggles, prompt-and-remember settings |
| [Refactoring](todo/refactor.md) | Technical debt and cleanup tasks. Gate check between sprints: clear all items before starting the next sprint |
| [Indexing](todo/indexing.md) | Self-generated classmap, staleness detection, parallel file processing, full background indexing, disk cache |
| [External Stubs](todo/external-stubs.md) | Composer stub discovery, IDE-provided stub paths, GTD for built-in symbols, stub override priority, SPL overlay stubs |
| [Performance](todo/performance.md) | FQN index, `Arc<ClassInfo>`, `RwLock`, inheritance dedup, file content cloning, type substitution optimisation |

---

## Sprint 3 — Refactoring & deferred performance

Extract Function is the remaining refactoring pillar. Inline Variable,
Extract Variable, and Inline Function/Method round out the core
refactoring toolkit, sharing scope analysis infrastructure with Extract
Function. Find References and Rename provide the variable/symbol usage
tracking infrastructure they depend on, and both are now complete.

The deferred performance items from Sprint 2.5 are included here. They
improve lookup speed, reduce cloning overhead, and prepare the codebase
for full background indexing (Phase 5), but are not prerequisites for
parallel file processing.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 17 | Extract Function refactoring | Very High | Code Actions | [actions.md §3](todo/actions.md#3-extract-function-refactoring) |
| 76 | Inline Variable | Medium | Code Actions | [actions.md §7](todo/actions.md#7-inline-variable) |
| 77 | Extract Variable | Medium | Code Actions | [actions.md §8](todo/actions.md#8-extract-variable) |
| 78 | Inline Function/Method | High | Code Actions | [actions.md §9](todo/actions.md#9-inline-functionmethod) |
| 87 | Reference-counted `ClassInfo` (`Arc<ClassInfo>`) | Medium | Performance | [performance.md §2](todo/performance.md#2-reference-counted-classinfo-arcclassinfo) |
| 88 | Early-exit and `Cow` return in `apply_substitution` | Low | Performance | [performance.md §7](todo/performance.md#7-recursive-string-substitution-in-apply_substitution) |

---

## Sprint 4 — Close the LSP feature gap

These items close the most commonly expected LSP feature surface gaps.
Each one removes a reason someone might look elsewhere.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 19 | Document Symbols (`textDocument/documentSymbol`) | Low | LSP Features | [lsp-features.md §4](todo/lsp-features.md#4-document-symbols-textdocumentdocumentsymbol) |
| 20 | Workspace Symbols (`workspace/symbol`) | Low-Medium | LSP Features | [lsp-features.md §5](todo/lsp-features.md#5-workspace-symbols-workspacesymbol) |
| 21 | Folding Ranges (`textDocument/foldingRange`) | Low | LSP Features | [lsp-features.md §12](todo/lsp-features.md#12-folding-ranges-textdocumentfoldingrange) |
| 22 | Selection Ranges (`textDocument/selectionRange`) | Low | LSP Features | [lsp-features.md §13](todo/lsp-features.md#13-selection-ranges-textdocumentselectionrange) |
| 23 | Type Definition (`textDocument/typeDefinition`) | Low | LSP Features | [lsp-features.md §14](todo/lsp-features.md#14-type-definition-textdocumenttypedefinition) |
| 24 | PHPDoc block generation on `/**` | Medium | LSP Features | [lsp-features.md §3](todo/lsp-features.md#3-phpdoc-block-generation-on-) |
| 81 | Work-done progress for GTI and Find References | Low | LSP Features | [lsp-features.md §18](todo/lsp-features.md#18-work-done-progress-for-gti-and-find-references) |

**After Sprint 4:** PHPantom covers every commonly expected LSP feature
and surpasses the field on type intelligence, generics, Laravel, and
performance. No feature gaps remain for typical day-to-day editing.

---

## Sprint 5 — Type intelligence depth & polish

Type intelligence depth is PHPantom's defining advantage. This sprint
deepens that lead and rounds out the remaining feature surface.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 26 | Inherited docblock type propagation | Medium | Type Inference | [type-inference.md §4](todo/type-inference.md#4-inherited-docblock-type-propagation) |
| 27 | `BackedEnum::from()` / `::tryFrom()` return type refinement | Low | Completion | [completion.md §1](todo/completion.md#1-backedenumfrom--tryfrom-return-type-refinement) |
| 28 | Pipe operator (PHP 8.5) type resolution | Low | Type Inference | [type-inference.md §1](todo/type-inference.md#1-pipe-operator-php-85) |
| 29 | Conditional return types `($param is T ? A : B)` | Medium | Type Inference | [type-inference.md §3](todo/type-inference.md#3-parse-and-resolve-param-is-t--a--b-return-types) |
| 30 | `@param-closure-this` | Medium | Type Inference | [type-inference.md §15](todo/type-inference.md#15-param-closure-this) |
| 31 | `key-of<T>` and `value-of<T>` resolution | Medium | Type Inference | [type-inference.md §16](todo/type-inference.md#16-key-oft-and-value-oft-resolution) |
| 32 | Code Lens: jump to prototype method | Low | LSP Features | [lsp-features.md §8](todo/lsp-features.md#8-code-lens-jump-to-prototype-method) |
| 34 | Document Links (`textDocument/documentLink`) | Low | LSP Features | [lsp-features.md §15](todo/lsp-features.md#15-document-links-textdocumentdocumentlink) |
| 35 | Resolution-failure diagnostics (unresolved function, unresolved PHPDoc type) | Medium | Diagnostics | [diagnostics.md §2](todo/diagnostics.md#2-resolution-failure-diagnostics) |
| 36 | Warn when composer.json is missing or classmap not optimized | Medium | Diagnostics | [diagnostics.md §5](todo/diagnostics.md#5-warn-when-composerjson-is-missing-or-classmap-is-not-optimized) |
| 37 | File system watching for vendor and project changes | Medium | Type Inference | [type-inference.md §5](todo/type-inference.md#5-file-system-watching-for-vendor-and-project-changes) |
| 38 | Property hooks (PHP 8.4) | Medium | Type Inference | [type-inference.md §6](todo/type-inference.md#6-property-hooks-php-84) |
| 39 | Simplify with null coalescing / null-safe operator (code action) | Medium | Code Actions | [actions.md §2](todo/actions.md#2-simplify-with-null-coalescing--null-safe-operator) |
| 40 | Inlay hints (`textDocument/inlayHint`) | Medium | LSP Features | [lsp-features.md §9](todo/lsp-features.md#9-inlay-hints-textdocumentinlayhint) |
| 91 | GTD for built-in symbols via project-level phpstorm-stubs | Low | External Stubs | [external-stubs.md §1](todo/external-stubs.md#phase-1-project-level-phpstorm-stubs-for-gtd) |

**After Sprint 5:** PHPantom has a complete, polished LSP feature set.
Users moving to Zed/Neovim/Helix lose nothing on the intelligence side
and gain 1000× faster startup. The remaining gaps are Blade and
formatting (not our domain).

---

## Sprint 6 — Deep type accuracy & Laravel excellence

These items push type resolution accuracy beyond what any tool offers.
They're the long tail that makes PHPantom the definitive choice for
projects that care about types.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 44 | Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`) | Medium | Laravel | [laravel.md §3](todo/laravel.md#3-custom-eloquent-builders-hasbuilder--useeloquentbuilder) |
| 45 | `abort_if`/`abort_unless` type narrowing | Medium | Laravel | [laravel.md §4](todo/laravel.md#4-abort_ifabort_unless-type-narrowing) |
| 46 | Narrow types of `&$var` parameters after function calls | Medium | Type Inference | [type-inference.md §7](todo/type-inference.md#7-narrow-types-of-var-parameters-after-function-calls) |
| 47 | SPL iterator generic stubs | Medium | Type Inference | [type-inference.md §8](todo/type-inference.md#8-spl-iterator-generic-stubs) |
| 48 | `LanguageLevelTypeAware` version-aware type hints | Medium | Completion | [completion.md §2](todo/completion.md#2-languageleveltypeaware-version-aware-type-hints) |
| 49 | `#[ArrayShape]` return shapes on stub functions | Medium | Completion | [completion.md §3](todo/completion.md#3-arrayshape-return-shapes-on-stub-functions) |
| 50 | Asymmetric visibility (PHP 8.4) | Low | Type Inference | [type-inference.md §9](todo/type-inference.md#9-asymmetric-visibility-php-84) |
| 51 | Type Hierarchy (`textDocument/prepareTypeHierarchy`) | Medium | LSP Features | [lsp-features.md §16](todo/lsp-features.md#16-type-hierarchy-textdocumentpreparetypehierarchy) |
| 52 | `class_alias()` support | Medium | Completion | [completion.md §8](todo/completion.md#8-class_alias-support) |
| 53 | Attribute constructor signature help | Medium | Signature Help | [signature-help.md §4](todo/signature-help.md#4-attribute-constructor-signature-help) |
| 54 | Closure/arrow function parameter signature help | Medium | Signature Help | [signature-help.md §5](todo/signature-help.md#5-closure--arrow-function-parameter-signature-help) |
| 55 | Diagnostic suppression intelligence | Medium | Diagnostics | [diagnostics.md §3](todo/diagnostics.md#3-diagnostic-suppression-intelligence) |
| 56 | Partial result streaming via `$/progress` | Medium-High | LSP Features | [lsp-features.md §6](todo/lsp-features.md#6-partial-result-streaming-via-progress) |

**Note:** Item 51 (Type Hierarchy) depends on the go-to-implementation
infrastructure and should be scheduled after that work is stable. Item 56
(partial result streaming) addresses outbound latency for large result
sets. See also item 89 (incremental text sync) in the backlog, which
addresses the complementary inbound direction.

---

## Sprint 7 — Blade support

Blade is a multi-phase project tracked in [todo/blade.md](todo/blade.md).
Shipping Blade support makes PHPantom the first open-source PHP language
server with Blade intelligence.

| Phase | Scope | Key Items |
|---|---|---|
| Phase 1 | Blade-to-PHP preprocessor | Module skeleton, directive translation, source map, LSP wiring |
| Phase 2 | Component support | Template/component discovery, `<x-component>` parsing, `@props`/`@aware`, name completion |
| Phase 3 | Cross-file view intelligence | View name GTD, signature merging for `@extends`, component→template variable typing |
| Phase 4 | Blade directive completion | Directive name completion with snippet insertion |

---

## Backlog — Diminishing returns

These items improve accuracy in niche scenarios. They're worth doing
eventually but don't move the needle.

### Completion & type inference tail

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 58 | Array functions needing new code paths | High | Completion | [completion.md §1](todo/completion.md#1-array-functions-needing-new-code-paths) |
| 59 | Go-to-definition for array shape keys via bracket access | Medium | Completion | [completion.md §4](todo/completion.md#4-go-to-definition-for-array-shape-keys-via-bracket-access) |
| 60 | No go-to-definition for built-in (stub) functions and constants — superseded by item 91 | Medium | LSP Features | [external-stubs.md §1](todo/external-stubs.md#phase-1-project-level-phpstorm-stubs-for-gtd) |
| 61 | `str_contains` / `str_starts_with` / `str_ends_with` → non-empty-string narrowing | Low | Type Inference | [type-inference.md §10](todo/type-inference.md#10-str_contains--str_starts_with--str_ends_with--non-empty-string-narrowing) |
| 62 | `count` / `sizeof` comparison → non-empty-array narrowing | Low | Type Inference | [type-inference.md §11](todo/type-inference.md#11-count--sizeof-comparison--non-empty-array-narrowing) |
| 63 | Fiber type resolution | Low | Type Inference | [type-inference.md §12](todo/type-inference.md#12-fiber-type-resolution) |
| 64 | Non-empty-string propagation through string functions | Low | Type Inference | [type-inference.md §13](todo/type-inference.md#13-non-empty-string-propagation-through-string-functions) |
| 65 | `Closure::bind()` / `Closure::fromCallable()` return type preservation | Low-Medium | Type Inference | [type-inference.md §14](todo/type-inference.md#14-closurebind--closurefromcallable-return-type-preservation) |
| 66 | Non-array functions with dynamic return types | High | Completion | [completion.md §5](todo/completion.md#5-non-array-functions-with-dynamic-return-types) |
| 67 | `#[ReturnTypeContract]` parameter-dependent return types | Low | Completion | [completion.md §6](todo/completion.md#6-returntypecontract-parameter-dependent-return-types) |
| 68 | `#[ExpectedValues]` parameter value suggestions | Medium | Completion | [completion.md §7](todo/completion.md#7-expectedvalues-parameter-value-suggestions) |

### Signature help polish

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 69 | Multiple overloaded signatures | Medium-High | Signature Help | [signature-help.md §7](todo/signature-help.md#7-multiple-overloaded-signatures) |
| 70 | Named argument awareness in active parameter | Medium | Signature Help | [signature-help.md §8](todo/signature-help.md#8-named-argument-awareness-in-active-parameter) |
| 71 | Language construct signature help and hover | Low | Signature Help | [signature-help.md §9](todo/signature-help.md#9-language-construct-signature-help-and-hover) |

### LSP features & code actions

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 72 | Switch → match conversion | Medium | Code Actions | [actions.md §4](todo/actions.md#4-switch--match-conversion) |
| 89 | Incremental text sync | Medium | Performance | [performance.md §8](todo/performance.md#8-incremental-text-sync) |

### Performance long-tail

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 90 | Type AST for `apply_substitution` (full refactor) | High | Performance | [performance.md §7](todo/performance.md#7-recursive-string-substitution-in-apply_substitution) |

### External stubs

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 92 | Project-level stubs as type resolution source | Medium | External Stubs | [external-stubs.md §2](todo/external-stubs.md#phase-2-project-level-stubs-as-resolution-source) |
| 93 | IDE-provided and `.phpantom.toml` stub paths | Low | External Stubs | [external-stubs.md §3](todo/external-stubs.md#phase-3-ide-provided-and-phpantomtoml-stub-paths) |
| 94 | Ship SPL overlay stubs, let external stubs override | Low | External Stubs | [external-stubs.md §4](todo/external-stubs.md#phase-4-embedded-stub-override-with-external-stubs) |

---

## Laravel-Specific Gaps

These are tracked in [todo/laravel.md](todo/laravel.md) and ranked
separately by their own impact÷effort scoring.

| # | Item | Impact | Effort | Doc Link |
|---|---|---|---|---|
| L1 | `morphedByMany` missing from relationship method map | ★★ | ★ | [§1](todo/laravel.md#1-morphedbymany-missing-from-relationship-method-map) |
| L2 | `$dates` array (deprecated) | ★★ | ★★ | [§2](todo/laravel.md#2-dates-array-deprecated) |
| L3 | Custom Eloquent builders | ★★★★ | ★★★ | [§3](todo/laravel.md#3-custom-eloquent-builders-hasbuilder--useeloquentbuilder) |
| L4 | `abort_if`/`abort_unless` type narrowing | ★★★★ | ★★★ | [§4](todo/laravel.md#4-abort_ifabort_unless-type-narrowing) |
| L5 | `collect()` generic type info | ★★★★★ | ★★★★ | [§5](todo/laravel.md#5-collect-and-other-helper-functions-lose-generic-type-info) |
| L6 | Factory `has*`/`for*` relationship methods | ★★ | ★★★ | [§6](todo/laravel.md#6-factory-hasfor-relationship-methods) |
| L7 | `$pivot` property on BelongsToMany | ★★★ | ★★★★ | [§7](todo/laravel.md#7-pivot-property-on-belongstomany-related-models) |
| L8 | `withSum`/`withAvg`/`withMin`/`withMax` aggregate properties | ★★ | ★★★★ | [§8](todo/laravel.md#8-withsum--withavg--withmin--withmax-aggregate-properties) |
| L9 | Higher-order collection proxies | ★★ | ★★★★ | [§9](todo/laravel.md#9-higher-order-collection-proxies) |
| L11 | `View::withX()` / `RedirectResponse::withX()` dynamic methods | ★ | ★★ | [§11](todo/laravel.md#11-viewwithx-and-redirectresponsewithx-dynamic-methods) |
| L12 | `$appends` array | ★ | ★ | [§12](todo/laravel.md#12-appends-array) |

---

## Blade Support

Blade is a multi-phase project tracked in [todo/blade.md](todo/blade.md).

| Phase | Scope | Key Items |
|---|---|---|
| Phase 1 | Blade-to-PHP preprocessor | Module skeleton, directive translation, source map, LSP wiring |
| Phase 2 | Component support | Template/component discovery, `<x-component>` parsing, `@props`/`@aware`, name completion |
| Phase 3 | Cross-file view intelligence | View name GTD, signature merging for `@extends`, component→template variable typing |
| Phase 4 | Blade directive completion | Directive name completion with snippet insertion |