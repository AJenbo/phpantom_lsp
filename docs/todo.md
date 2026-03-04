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
| [Type Inference](todo-type-inference.md) | Generic resolution, conditional return types, type narrowing, stub attribute handling |
| [Completion](todo-completion.md) | Completion-specific improvements (enum return types, array shapes, expected values) |
| [Diagnostics](todo-diagnostics.md) | `@deprecated` warnings, resolution-failure diagnostics, unused `use` dimming, suppression intelligence |
| [Code Actions](todo-actions.md) | Import class, remove unused imports, implement missing methods, null coalescing simplification, extract function, switch→match |
| [LSP Features](todo-lsp-features.md) | Find references, document highlighting, document/workspace symbols, rename, code lens, inlay hints, PHPDoc generation, partial result streaming |
| [Hover](todo-hover.md) | Deprecation messages, constant values, member origin indicators, enum case listing, trait method summaries |
| [Signature Help](todo-signatureHelp.md) | Parameter descriptions, signature-level docs, default values, attribute/closure support |
| [Laravel](todo-laravel.md) | Model property gaps, relationship methods, type narrowing, custom builders |
| [Blade](todo-blade.md) | Preprocessor, component support, cross-file view intelligence |
| [Testing](todo-testing.md) | Fixture runner, Phpactor test mining, benchmarks |
| [Bug Fixes](todo-bugs.md) | Incorrect behaviour that should be fixed regardless of feature priority |

---

## Sprint 1 — Code actions (imports & diagnostics)

Ship the first code actions. Import management is the single most
requested code action in any PHP language server. Pair it with the
cheapest diagnostics for maximum visual impact.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 10 | Import class (code action) | Medium | Code Actions | [todo-actions.md §5](todo-actions.md#5-import-class) |
| 11 | Remove unused imports (code action) | Low | Code Actions | [todo-actions.md §6](todo-actions.md#6-remove-unused-imports) |
| 12 | Unused `use` dimming | Low | Diagnostics | [todo-diagnostics.md §4](todo-diagnostics.md#4-unused-use-dimming) |
| 13 | `@deprecated` usage diagnostics | Low | Diagnostics | [todo-diagnostics.md §1](todo-diagnostics.md#1-deprecated-usage-diagnostics) |
| 14 | Implement missing abstract/interface methods (code action) | Medium | Code Actions | [todo-actions.md §1](todo-actions.md#1-implement-missing-abstractinterface-methods) |

**Why this order:** Items 10-11 are the bread-and-butter code actions
that every developer uses daily. Item 12 pairs naturally with item 11
(dimmed imports that can be removed with a quick-fix). Item 13 adds
strikethrough on deprecated calls, signalling "production-ready" to new
users. Item 14 builds on the code action infrastructure from 10-11.

---

## Sprint 2 — Refactoring & references

Rename and Extract Function are the two refactoring pillars. Find
References is a prerequisite for Rename and broadly useful on its own.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 15 | Find References (`textDocument/references`) | Medium-High | LSP Features | [todo-lsp-features.md §1](todo-lsp-features.md#1-find-references-textdocumentreferences) |
| 16 | Rename (`textDocument/rename`) | Medium-High | LSP Features | [todo-lsp-features.md §7](todo-lsp-features.md#7-rename-textdocumentrename) |
| 17 | Extract Function refactoring | Very High | Code Actions | [todo-actions.md §3](todo-actions.md#3-extract-function-refactoring) |

**Why this order:** Find References (15) provides the variable/symbol
usage tracking infrastructure that Rename (16) and Extract Function (17)
both depend on. Rename is a smaller step that validates the
infrastructure before tackling Extract Function.

---

## Sprint 3 — Close the LSP feature gap

These items close the most commonly expected LSP feature surface gaps.
Each one removes a reason someone might look elsewhere.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 18 | Document Highlighting (`textDocument/documentHighlight`) | Low | LSP Features | [todo-lsp-features.md §2](todo-lsp-features.md#2-document-highlighting-textdocumentdocumenthighlight) |
| 19 | Document Symbols (`textDocument/documentSymbol`) | Low | LSP Features | [todo-lsp-features.md §4](todo-lsp-features.md#4-document-symbols-textdocumentdocumentsymbol) |
| 20 | Workspace Symbols (`workspace/symbol`) | Low-Medium | LSP Features | [todo-lsp-features.md §5](todo-lsp-features.md#5-workspace-symbols-workspacesymbol) |
| 21 | Folding Ranges (`textDocument/foldingRange`) | Low | LSP Features | [todo-lsp-features.md §12](todo-lsp-features.md#12-folding-ranges-textdocumentfoldingrange) |
| 22 | Selection Ranges (`textDocument/selectionRange`) | Low | LSP Features | [todo-lsp-features.md §13](todo-lsp-features.md#13-selection-ranges-textdocumentselectionrange) |
| 23 | Type Definition (`textDocument/typeDefinition`) | Low | LSP Features | [todo-lsp-features.md §14](todo-lsp-features.md#14-type-definition-textdocumenttypedefinition) |
| 24 | PHPDoc block generation on `/**` | Medium | LSP Features | [todo-lsp-features.md §3](todo-lsp-features.md#3-phpdoc-block-generation-on-) |

**After Sprint 3:** PHPantom covers every commonly expected LSP feature
and surpasses the field on type intelligence, generics, Laravel, and
performance. No feature gaps remain for typical day-to-day editing.

---

## Sprint 4 — Type intelligence depth & polish

Type intelligence depth is PHPantom's defining advantage. This sprint
deepens that lead and rounds out the remaining feature surface.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 25 | Function-level `@template` generic resolution | Medium | Type Inference | [todo-type-inference.md §2](todo-type-inference.md#2-function-level-template-generic-resolution) |
| 26 | Inherited docblock type propagation | Medium | Type Inference | [todo-type-inference.md §4](todo-type-inference.md#4-inherited-docblock-type-propagation) |
| 27 | `BackedEnum::from()` / `::tryFrom()` return type refinement | Low | Completion | [todo-completion.md §1](todo-completion.md#1-backedenumfrom--tryfrom-return-type-refinement) |
| 28 | Pipe operator (PHP 8.5) type resolution | Low | Type Inference | [todo-type-inference.md §1](todo-type-inference.md#1-pipe-operator-php-85) |
| 29 | Conditional return types `($param is T ? A : B)` | Medium | Type Inference | [todo-type-inference.md §3](todo-type-inference.md#3-parse-and-resolve-param-is-t--a--b-return-types) |
| 30 | `@param-closure-this` | Medium | Type Inference | [todo-type-inference.md §15](todo-type-inference.md#15-param-closure-this) |
| 31 | `key-of<T>` and `value-of<T>` resolution | Medium | Type Inference | [todo-type-inference.md §16](todo-type-inference.md#16-key-oft-and-value-oft-resolution) |
| 32 | Code Lens: jump to prototype method | Low | LSP Features | [todo-lsp-features.md §8](todo-lsp-features.md#8-code-lens-jump-to-prototype-method) |
| 33 | Implementation → interface method declaration (reverse jump) | Low | LSP Features | [todo-lsp-features.md §10](todo-lsp-features.md#10-reverse-jump-implementation--interface-method-declaration) |
| 34 | Document Links (`textDocument/documentLink`) | Low | LSP Features | [todo-lsp-features.md §15](todo-lsp-features.md#15-document-links-textdocumentdocumentlink) |
| 35 | Resolution-failure diagnostics | Medium | Diagnostics | [todo-diagnostics.md §2](todo-diagnostics.md#2-resolution-failure-diagnostics) |
| 36 | Warn when composer.json is missing or classmap not optimized | Medium | Diagnostics | [todo-diagnostics.md §5](todo-diagnostics.md#5-warn-when-composerjson-is-missing-or-classmap-is-not-optimized) |
| 37 | File system watching for vendor and project changes | Medium | Type Inference | [todo-type-inference.md §5](todo-type-inference.md#5-file-system-watching-for-vendor-and-project-changes) |
| 38 | Property hooks (PHP 8.4) | Medium | Type Inference | [todo-type-inference.md §6](todo-type-inference.md#6-property-hooks-php-84) |
| 39 | Simplify with null coalescing / null-safe operator (code action) | Medium | Code Actions | [todo-actions.md §2](todo-actions.md#2-simplify-with-null-coalescing--null-safe-operator) |
| 40 | Inlay hints (`textDocument/inlayHint`) | Medium | LSP Features | [todo-lsp-features.md §9](todo-lsp-features.md#9-inlay-hints-textdocumentinlayhint) |

**After Sprint 5:** PHPantom has a complete, polished LSP feature set.
Users moving to Zed/Neovim/Helix lose nothing on the intelligence side
and gain 1000× faster startup. The remaining gaps are Blade and
formatting (not our domain).

---

## Sprint 5 — Deep type accuracy & Laravel excellence

These items push type resolution accuracy beyond what any tool offers.
They're the long tail that makes PHPantom the definitive choice for
projects that care about types.

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 41 | `collect()` and helper functions lose generic type info | High | Laravel | [todo-laravel.md §5](todo-laravel.md#5-collect-and-other-helper-functions-lose-generic-type-info) |
| 42 | Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`) | Medium | Laravel | [todo-laravel.md §3](todo-laravel.md#3-custom-eloquent-builders-hasbuilder--useeloquentbuilder) |
| 43 | `abort_if`/`abort_unless` type narrowing | Medium | Laravel | [todo-laravel.md §4](todo-laravel.md#4-abort_ifabort_unless-type-narrowing) |
| 44 | Narrow types of `&$var` parameters after function calls | Medium | Type Inference | [todo-type-inference.md §7](todo-type-inference.md#7-narrow-types-of-var-parameters-after-function-calls) |
| 45 | SPL iterator generic stubs | Medium | Type Inference | [todo-type-inference.md §8](todo-type-inference.md#8-spl-iterator-generic-stubs) |
| 46 | `LanguageLevelTypeAware` version-aware type hints | Medium | Completion | [todo-completion.md §3](todo-completion.md#3-languageleveltypeaware-version-aware-type-hints) |
| 47 | `#[ArrayShape]` return shapes on stub functions | Medium | Completion | [todo-completion.md §4](todo-completion.md#4-arrayshape-return-shapes-on-stub-functions) |
| 48 | `#[Deprecated]` structured deprecation metadata | Low | Completion | [todo-completion.md §5](todo-completion.md#5-deprecated-structured-deprecation-metadata) |
| 49 | Asymmetric visibility (PHP 8.4) | Low | Type Inference | [todo-type-inference.md §9](todo-type-inference.md#9-asymmetric-visibility-php-84) |
| 50 | Type Hierarchy (`textDocument/prepareTypeHierarchy`) | Medium | LSP Features | [todo-lsp-features.md §16](todo-lsp-features.md#16-type-hierarchy-textdocumentpreparetypehierarchy) |
| 51 | `class_alias()` support | Medium | Completion | [todo-completion.md §10](todo-completion.md#10-class_alias-support) |
| 52 | Attribute constructor signature help | Medium | Signature Help | [todo-signatureHelp.md §4](todo-signatureHelp.md#4-attribute-constructor-signature-help) |
| 53 | Closure/arrow function parameter signature help | Medium | Signature Help | [todo-signatureHelp.md §5](todo-signatureHelp.md#5-closure--arrow-function-parameter-signature-help) |
| 54 | Diagnostic suppression intelligence | Medium | Diagnostics | [todo-diagnostics.md §3](todo-diagnostics.md#3-diagnostic-suppression-intelligence) |
| 55 | Partial result streaming via `$/progress` | Medium-High | LSP Features | [todo-lsp-features.md §6](todo-lsp-features.md#6-partial-result-streaming-via-progress) |

**Note:** Item 41 (`collect()` generics) is a direct payoff of Sprint 5
item 25 (function-level `@template`). Once the infrastructure exists,
the Laravel-specific manifestation is a small incremental step. Item 50
(Type Hierarchy) depends on the go-to-implementation infrastructure and
should be scheduled after that work is stable. Item 55 (partial result
streaming) addresses outbound latency for large result sets. See also
item 73 (incremental text sync) in the backlog, which addresses the
complementary inbound direction.

---

## Sprint 6 — Blade support

Blade is a multi-phase project tracked in [todo-blade.md](todo-blade.md).
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
| 56 | Array functions needing new code paths | High | Completion | [todo-completion.md §2](todo-completion.md#2-array-functions-needing-new-code-paths) |
| 57 | Go-to-definition for array shape keys via bracket access | Medium | Completion | [todo-completion.md §6](todo-completion.md#6-go-to-definition-for-array-shape-keys-via-bracket-access) |
| 58 | No go-to-definition for built-in (stub) functions and constants | Medium | LSP Features | [todo-lsp-features.md §11](todo-lsp-features.md#11-no-go-to-definition-for-built-in-stub-functions-and-constants) |
| 59 | `str_contains` / `str_starts_with` / `str_ends_with` → non-empty-string narrowing | Low | Type Inference | [todo-type-inference.md §10](todo-type-inference.md#10-str_contains--str_starts_with--str_ends_with--non-empty-string-narrowing) |
| 60 | `count` / `sizeof` comparison → non-empty-array narrowing | Low | Type Inference | [todo-type-inference.md §11](todo-type-inference.md#11-count--sizeof-comparison--non-empty-array-narrowing) |
| 61 | Fiber type resolution | Low | Type Inference | [todo-type-inference.md §12](todo-type-inference.md#12-fiber-type-resolution) |
| 62 | Non-empty-string propagation through string functions | Low | Type Inference | [todo-type-inference.md §13](todo-type-inference.md#13-non-empty-string-propagation-through-string-functions) |
| 63 | `Closure::bind()` / `Closure::fromCallable()` return type preservation | Low-Medium | Type Inference | [todo-type-inference.md §14](todo-type-inference.md#14-closurebind--closurefromcallable-return-type-preservation) |
| 64 | Non-array functions with dynamic return types | High | Completion | [todo-completion.md §7](todo-completion.md#7-non-array-functions-with-dynamic-return-types) |
| 65 | `#[ReturnTypeContract]` parameter-dependent return types | Low | Completion | [todo-completion.md §8](todo-completion.md#8-returntypecontract-parameter-dependent-return-types) |
| 66 | `#[ExpectedValues]` parameter value suggestions | Medium | Completion | [todo-completion.md §9](todo-completion.md#9-expectedvalues-parameter-value-suggestions) |

### Signature help polish

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 67 | Multiple overloaded signatures | Medium-High | Signature Help | [todo-signatureHelp.md §7](todo-signatureHelp.md#7-multiple-overloaded-signatures) |
| 68 | Named argument awareness in active parameter | Medium | Signature Help | [todo-signatureHelp.md §8](todo-signatureHelp.md#8-named-argument-awareness-in-active-parameter) |
| 69 | Language construct signature help and hover | Low | Signature Help | [todo-signatureHelp.md §9](todo-signatureHelp.md#9-language-construct-signature-help-and-hover) |

### LSP features & code actions

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 70 | Switch → match conversion | Medium | Code Actions | [todo-actions.md §4](todo-actions.md#4-switch--match-conversion) |
| 71 | Incremental text sync | Medium | LSP Features | [todo-lsp-features.md §17](todo-lsp-features.md#17-incremental-text-sync) |

### Bug fixes

| # | Item | Effort | Domain | Doc Link |
|---|---|---|---|---|
| 72 | Short-name collisions in `find_implementors` | Low | Bug Fixes | [todo-bugs.md §1](todo-bugs.md#1-short-name-collisions-in-find_implementors) |

---

## Laravel-Specific Gaps

These are tracked in [todo-laravel.md](todo-laravel.md) and ranked
separately by their own impact÷effort scoring.

| # | Item | Impact | Effort | Doc Link |
|---|---|---|---|---|
| L1 | `morphedByMany` missing from relationship method map | ★★ | ★ | [§1](todo-laravel.md#1-morphedbymany-missing-from-relationship-method-map) |
| L2 | `$dates` array (deprecated) | ★★ | ★★ | [§2](todo-laravel.md#2-dates-array-deprecated) |
| L3 | Custom Eloquent builders | ★★★★ | ★★★ | [§3](todo-laravel.md#3-custom-eloquent-builders-hasbuilder--useeloquentbuilder) |
| L4 | `abort_if`/`abort_unless` type narrowing | ★★★★ | ★★★ | [§4](todo-laravel.md#4-abort_ifabort_unless-type-narrowing) |
| L5 | `collect()` generic type info | ★★★★★ | ★★★★ | [§5](todo-laravel.md#5-collect-and-other-helper-functions-lose-generic-type-info) |
| L6 | Factory `has*`/`for*` relationship methods | ★★ | ★★★ | [§6](todo-laravel.md#6-factory-hasfor-relationship-methods) |
| L7 | `$pivot` property on BelongsToMany | ★★★ | ★★★★ | [§7](todo-laravel.md#7-pivot-property-on-belongstomany-related-models) |
| L8 | `withSum`/`withAvg`/`withMin`/`withMax` aggregate properties | ★★ | ★★★★ | [§8](todo-laravel.md#8-withsum--withavg--withmin--withmax-aggregate-properties) |
| L9 | Higher-order collection proxies | ★★ | ★★★★ | [§9](todo-laravel.md#9-higher-order-collection-proxies) |
| L10 | `SoftDeletes` trait methods on Builder | ★ | ★ | [§10](todo-laravel.md#10-softdeletes-trait-methods-on-builder) |
| L11 | `View::withX()` / `RedirectResponse::withX()` dynamic methods | ★ | ★★ | [§11](todo-laravel.md#11-viewwithx-and-redirectresponsewithx-dynamic-methods) |
| L12 | `$appends` array | ★ | ★ | [§12](todo-laravel.md#12-appends-array) |
| L13 | Relationship classification matches short name only | ★ | ★★ | [§13](todo-laravel.md#13-relationship-classification-matches-short-name-only) |

---

## Blade Support

Blade is a multi-phase project tracked in [todo-blade.md](todo-blade.md).

| Phase | Scope | Key Items |
|---|---|---|
| Phase 1 | Blade-to-PHP preprocessor | Module skeleton, directive translation, source map, LSP wiring |
| Phase 2 | Component support | Template/component discovery, `<x-component>` parsing, `@props`/`@aware`, name completion |
| Phase 3 | Cross-file view intelligence | View name GTD, signature merging for `@extends`, component→template variable typing |
| Phase 4 | Blade directive completion | Directive name completion with snippet insertion |

---

## Testing

Testing infrastructure and Phpactor fixture mining are tracked in
[todo-testing.md](todo-testing.md).

| Phase | Scope |
|---|---|
| Phase 1 | Build a fixture runner |
| Phase 2 | Audit Phpactor's fixtures against our coverage |
| Phase 3 | Convert high-value fixtures |
| Phase 4 | Mine Phpactor's completion tests |
| Phase 5 | Smoke tests and benchmarks |