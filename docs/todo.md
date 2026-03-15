# PHPantom — Roadmap

This is the master index for all planned work. Each row links to the
domain document that contains the full specification. Items are
sequenced by **sprint priority** — what to build next to widen the type
intelligence lead, then close the LSP feature gap, maximising coverage
with each step.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

**Task ID prefixes:** Each domain document has its own prefix letter so
IDs are stable across renumbering. **C** = Completion, **T** = Type
Inference, **D** = Diagnostics, **A** = Code Actions, **F** = LSP
Features, **S** = Signature Help, **L** = Laravel, **E** = External
Stubs, **P** = Performance, **X** = Indexing, **B** = Bug Fixes,
**N** = Inline Completion.

---

# Scheduled Sprints

## Sprint 3 — Quick wins: close the visible gaps

Every item here directly removes something a Neovim/Zed/VS Code user
would notice as missing on day one. PHPDoc block generation is the
most common "where did that go?" moment for users switching from
PHPStorm or Intelephense. Update Docblock (A8) is a natural companion
to PHPDoc generation (F1) since they share docblock manipulation code,
and Change Visibility (A9) is a low-effort quick win. Neither depends
on scope analysis or the refactoring toolkit, so they belong here
rather than waiting for later sprints.

| #   | Item                                                              | Effort | Domain       | Doc Link                                                                              |
| --- | ----------------------------------------------------------------- | ------ | ------------ | ------------------------------------------------------------------------------------- |
| P9  | Pull diagnostics (`textDocument/diagnostic`) — replace push model | Medium | Performance  | [performance.md P9](todo/performance.md#p9-pull-diagnostics-textdocumentdiagnostic)   |
| D7  | Implementation error diagnostic                                   | Medium | Diagnostics  | [diagnostics.md D7](todo/diagnostics.md#d7-implementation-error-diagnostic)           |
| D9  | Syntax error diagnostic (surface Mago parse errors)               | Low    | Diagnostics  | [diagnostics.md D9](todo/diagnostics.md#d9-syntax-error-diagnostic)                   |
| D1  | Scalar member access diagnostic — remaining gaps                  | Low    | Diagnostics  | [diagnostics.md D1](todo/diagnostics.md#d1-scalar-member-access-diagnostic--remaining-gaps) |
| F1  | PHPDoc block generation on `/**`                                  | Medium | LSP Features | [lsp-features.md F1](todo/lsp-features.md#f1-phpdoc-block-generation-on-)             |
| A8  | Update docblock to match signature                                | Medium | Code Actions | [actions.md A8](todo/actions.md#a8-update-docblock-to-match-signature)                |
| A9  | Change visibility                                                 | Low    | Code Actions | [actions.md A9](todo/actions.md#a9-change-visibility)                                 |

**After Sprint 3:** PHPantom feels like a complete LSP to everyday
users. Outline, breadcrumbs, workspace search, semantic highlighting,
folding, formatting, inlay hints, type hierarchy, document links,
and smart select all work. PHPDoc generation and docblock maintenance
work. Basic code actions (change visibility) are in place. No one
says "it's missing X" for basic editing workflows.

---

## Sprint 4 — Refactoring toolkit

Extract Function is the #1 personal feature request and something
that was available before the switch to PHPantom. Inline Variable,
Extract Variable, and Inline Function/Method have been specifically
requested by the Neovim tester. These all share scope analysis
infrastructure, so the sprint begins by building the `ScopeCollector`
as standalone shared infrastructure. This de-risks Extract Function
and lets the other refactorings proceed independently if A2 stalls.

| #   | Item                         | Effort    | Domain       | Doc Link                                                         |
| --- | ---------------------------- | --------- | ------------ | ---------------------------------------------------------------- |
| —   | Clear refactoring gate       | —         | Refactoring  | [refactor.md](todo/refactor.md)                                  |
| A11 | `ScopeCollector` infrastructure (shared by A2/A4/A5/A6/A7) | Medium | Code Actions | [actions.md A11](todo/actions.md#a11-scopecollector-infrastructure) |
| A2  | Extract Function refactoring | High | Code Actions | [actions.md A2](todo/actions.md#a2-extract-function-refactoring) |
| A4  | Inline Variable              | Medium    | Code Actions | [actions.md A4](todo/actions.md#a4-inline-variable)              |
| A5  | Extract Variable             | Medium    | Code Actions | [actions.md A5](todo/actions.md#a5-extract-variable)             |
| A6  | Inline Function/Method       | High      | Code Actions | [actions.md A6](todo/actions.md#a6-inline-functionmethod)        |
| A7  | Extract Constant             | Medium    | Code Actions | [actions.md A7](todo/actions.md#a7-extract-constant)             |

**After Sprint 4:** The core refactoring toolkit is complete. The
two most active testers have the features they specifically asked
for. The `ScopeCollector` infrastructure also benefits D8 (undefined
variable diagnostic) in Sprint 5 and any future scope-aware features.

---

## Sprint 5 — Polish for office adoption

These items close the gaps that PHPStorm and VS Code + Intelephense
users at the office would notice. Undefined variable detection is the
single most impactful missing diagnostic (leveraging Sprint 4's
`ScopeCollector`). The two low-effort diagnostics (deprecated
rendering and unreachable code) round out the diagnostic story with
minimal cost. The null coalescing code action is independent of scope
analysis and grouped here by user-facing impact.

| #   | Item                                                             | Effort | Domain         | Doc Link                                                                                            |
| --- | ---------------------------------------------------------------- | ------ | -------------- | --------------------------------------------------------------------------------------------------- |
| —   | Clear refactoring gate                                           | —      | Refactoring    | [refactor.md](todo/refactor.md)                                                                     |
| D8  | Undefined variable diagnostic                                    | Medium | Diagnostics    | [diagnostics.md D8](todo/diagnostics.md#d8-undefined-variable-diagnostic)                           |
| D3  | Deprecated rendering gaps                                        | Low    | Diagnostics    | [diagnostics.md D3](todo/diagnostics.md#d3-deprecated-rendering)                                    |
| D6  | Unreachable code diagnostic                                      | Low    | Diagnostics    | [diagnostics.md D6](todo/diagnostics.md#d6-unreachable-code-diagnostic)                             |
| F4  | File rename on class rename                                      | Medium | LSP Features   | [lsp-features.md F4](todo/lsp-features.md#f4-file-rename-on-class-rename)                           |
| E5  | Stub extension selection (`[stubs] extensions`)                  | Low    | External Stubs | [external-stubs.md E5](todo/external-stubs.md#e5-extension-stub-selection-stubs-extensions)         |
| A1  | Simplify with null coalescing / null-safe operator (code action) | Medium | Code Actions   | [actions.md A1](todo/actions.md#a1-simplify-with-null-coalescing--null-safe-operator)               |

**After Sprint 5:** PHPantom is ready for office colleagues. They
get undefined variable detection, unreachable code dimming, proper
deprecated strikethrough, file rename on class rename, and the null
coalescing quick-fix. Nobody switching from Intelephense (free or
premium) feels like they lost more than they gained.

---

## Sprint 6 — Type intelligence depth

Type intelligence depth is PHPantom's defining advantage. This sprint
deepens that lead with features that benefit the PHPStan enthusiast
and Laravel developer alike. File system watching eliminates the
"restart the server after composer update" friction.

| #   | Item                                                        | Effort | Domain         | Doc Link                                                                                              |
| --- | ----------------------------------------------------------- | ------ | -------------- | ----------------------------------------------------------------------------------------------------- |
| —   | Clear refactoring gate                                      | —      | Refactoring    | [refactor.md](todo/refactor.md)                                                                       |
| T1  | Inherited docblock type propagation                         | Medium | Type Inference | [type-inference.md T1](todo/type-inference.md#t1-inherited-docblock-type-propagation)                 |
| T7  | `key-of<T>` and `value-of<T>` resolution                    | Medium | Type Inference | [type-inference.md T7](todo/type-inference.md#t7-key-oft-and-value-oft-resolution)                    |
| T2  | File system watching for vendor and project changes         | Medium | Type Inference | [type-inference.md T2](todo/type-inference.md#t2-file-system-watching-for-vendor-and-project-changes) |
| T3  | Property hooks and asymmetric visibility (PHP 8.4)          | Medium | Type Inference | [type-inference.md T3](todo/type-inference.md#t3-property-hooks-php-84)                               |
| D4  | Unresolved type in PHPDoc diagnostic                        | Medium | Diagnostics    | [diagnostics.md D4](todo/diagnostics.md#d4-unresolved-type-in-phpdoc)                                |
| E1  | GTD for built-in symbols via project-level phpstorm-stubs   | Low    | External Stubs | [external-stubs.md E1](todo/external-stubs.md#e1-project-level-phpstorm-stubs-for-gtd)               |

**After Sprint 6:** PHPantom has the deepest type intelligence of
any PHP language server. `key-of`/`value-of`, property hooks, and
inherited docblock types all work. The type engine advantage is
unambiguous.

---

## Sprint 7 — Laravel excellence & stub accuracy

Facade resolution (L1) is the highest-impact Laravel item remaining
and the primary interaction pattern for Laravel developers. The rest
of the sprint focuses on stub accuracy (version-aware types, array
shapes, SPL iterators) and signature help gaps. Partial result
streaming (F2) lands here to improve perceived latency for large
projects now that the feature set is mature enough to benefit from it.

| #   | Item                                                              | Effort | Domain         | Doc Link                                                                                              |
| --- | ----------------------------------------------------------------- | ------ | -------------- | ----------------------------------------------------------------------------------------------------- |
| —   | Clear refactoring gate                                            | —      | Refactoring    | [refactor.md](todo/refactor.md)                                                                       |
| L1  | Facade `getFacadeAccessor` resolution                             | Medium | Laravel        | [laravel.md L1](todo/laravel.md#l1-facade-completion)                                                |
| F2  | Partial result streaming via `$/progress`                         | Medium-High | LSP Features   | [lsp-features.md F2](todo/lsp-features.md#f2-partial-result-streaming-via-progress)                   |
| L5  | `abort_if`/`abort_unless` type narrowing                          | Medium | Laravel        | [laravel.md L5](todo/laravel.md#l5-abort_ifabort_unless-type-narrowing)                               |
| E4  | SPL iterator generic stubs (ship overlay stubs)                   | Medium | External Stubs | [external-stubs.md E4](todo/external-stubs.md#e4-embedded-stub-override-with-external-stubs)          |
| C2  | `LanguageLevelTypeAware` version-aware type hints                 | Medium | Completion     | [completion.md C2](todo/completion.md#c2-languageleveltypeaware-version-aware-type-hints)             |
| C3  | `#[ArrayShape]` return shapes on stub functions                   | Medium | Completion     | [completion.md C3](todo/completion.md#c3-arrayshape-return-shapes-on-stub-functions)                  |
| S1  | Attribute constructor signature help                              | Medium | Signature Help | [signature-help.md S1](todo/signature-help.md#s1-attribute-constructor-signature-help)                |
| S2  | Closure/arrow function parameter signature help                   | Medium | Signature Help | [signature-help.md S2](todo/signature-help.md#s2-closure--arrow-function-parameter-signature-help)    |

---

## Sprint 8 — Blade support

Blade is a multi-phase project tracked in [todo/blade.md](todo/blade.md).
Shipping Blade support makes PHPantom the first open-source PHP language
server with Blade intelligence.

| Phase   | Scope                        | Key Items                                                                                 |
| ------- | ---------------------------- | ----------------------------------------------------------------------------------------- |
| Phase 1 | Blade-to-PHP preprocessor    | Module skeleton, directive translation, source map, LSP wiring                            |
| Phase 2 | Component support            | Template/component discovery, `<x-component>` parsing, `@props`/`@aware`, name completion |
| Phase 3 | Cross-file view intelligence | View name GTD, signature merging for `@extends`, component→template variable typing       |
| Phase 4 | Blade directive completion   | Directive name completion with snippet insertion                                          |

---

# Backlog

Items not yet assigned to a sprint. Worth doing eventually but
unlikely to move the needle for most users.

| #   | Item                                                                    | Impact     | Effort      |
| --- | ----------------------------------------------------------------------- | ---------- | ----------- |
|     | **[Completion](todo/completion.md)**                                    |            |             |
| C1  | Array functions needing new code paths                                  | Medium     | High        |
| C4  | Go-to-definition for array shape keys via bracket access                | Low-Medium | Medium      |
| C5  | Non-array functions with dynamic return types                           | Low        | High        |
| C6  | `#[ReturnTypeContract]` parameter-dependent return types                | Low        | Low         |
| C7  | `#[ExpectedValues]` parameter value suggestions                         | Low        | Medium      |
| C8  | `class_alias()` support                                                 | Low-Medium | Medium      |
|     | **[Type Inference](todo/type-inference.md)**                            |            |             |
| T4  | Non-empty-* type narrowing and propagation                              | Low        | Low         |
| T5  | Fiber type resolution                                                   | Low        | Low         |
| T6  | `Closure::bind()` / `Closure::fromCallable()` return type preservation | Low-Medium | Low-Medium  |
|     | **[Diagnostics](todo/diagnostics.md)**                                  |            |             |
| D2  | Chain error propagation (flag only the first broken link)               | Medium     | Medium      |
| D5  | Diagnostic suppression intelligence                                     | Medium     | Medium      |
| D10 | PHPMD diagnostic proxy                                                  | Low        | Medium      |
|     | **[Code Actions](todo/actions.md)**                                     |            |             |
| A3  | Switch → match conversion                                               | Low        | Medium      |
| A10 | Generate interface from class                                           | Low-Medium | Medium      |
|     | **[LSP Features](todo/lsp-features.md)**                                |            |             |
| F3  | Incremental text sync                                                   | Low-Medium | Medium      |
|     | **[Signature Help](todo/signature-help.md)**                            |            |             |
| S3  | Multiple overloaded signatures                                          | Medium     | Medium-High |
| S4  | Named argument awareness in active parameter                            | Low-Medium | Medium      |
| S5  | Language construct signature help and hover                             | Low        | Low         |
|     | **[Laravel](todo/laravel.md)**                                          |            |             |
| L4  | Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`)       | Medium     | Medium      |
| L2  | `morphedByMany` missing from relationship method map                    | Low-Medium | Low         |
| L3  | `$dates` array (deprecated)                                             | Low-Medium | Low         |
| L6  | Factory `has*`/`for*` relationship methods                              | Low-Medium | Medium      |
| L7  | `$pivot` property on BelongsToMany                                      | Medium     | Medium-High |
| L8  | `withSum`/`withAvg`/`withMin`/`withMax` aggregate properties            | Low-Medium | Medium-High |
| L9  | Higher-order collection proxies                                         | Low-Medium | Medium-High |
| L10 | `View::withX()` / `RedirectResponse::withX()` dynamic methods          | Low        | Low         |
| L11 | `$appends` array                                                        | Low        | Low         |
|     | **[External Stubs](todo/external-stubs.md)**                            |            |             |
| E2  | Project-level stubs as type resolution source                           | Medium     | Medium      |
| E3  | IDE-provided and `.phpantom.toml` stub paths                            | Low-Medium | Low         |
| E6  | Stub install prompt for non-Composer projects                           | Low        | Low         |
|     | **[Performance](todo/performance.md)**                                  |            |             |
| P1  | Propagate `Arc<ClassInfo>` through downstream return types              | Medium     | Medium      |
| P2  | Type AST for `apply_substitution` (full refactor)                       | Medium     | High        |
| P3  | Parallel pre-filter in `find_implementors`                              | Low-Medium | Medium      |
| P4  | `memmem` for block comment terminator search                            | Low        | Low         |
| P5  | `memmap2` for file reads during scanning                                | Low        | Low         |
| P6  | O(n²) transitive eviction in `evict_fqn`                                | Low        | Low         |
| P7  | `diag_pending_uris` uses `Vec::contains` for dedup                      | Low        | Low         |
| P8  | `find_class_in_ast_map` linear fallback scan                            | Low        | Low         |
|     | **[Indexing](todo/indexing.md)**                                        |            |             |
| X1  | Staleness detection and auto-refresh                                    | Medium     | Medium      |
| X3  | Completion item detail on demand (`completionItem/resolve`)             | Medium     | Medium      |
| X2  | Parallel file processing — remaining work                               | Low-Medium | Medium      |
| X5  | Granular progress reporting for indexing, GTI, and Find References      | Low-Medium | Medium      |
| X4  | Full background indexing (`strategy = "full"`)                          | Medium     | High        |
| X6  | Disk cache (evaluate later)                                             | Medium     | High        |
|     | **[Bug Fixes](todo/bugs.md)**                                          |            |             |
| B1  | Native type hints not considered in virtual property specificity ranking | Low-Medium | Medium     |
|     | **[Inline Completion](todo/inline-completion.md)**                      |            |             |
| N1  | Template engine (type-aware snippets)                                   | Medium     | High        |
| N2  | N-gram prediction from PHP corpus                                       | Medium     | Very High   |
| N3  | Fine-tuned GGUF sidecar model                                           | Medium     | Very High   |