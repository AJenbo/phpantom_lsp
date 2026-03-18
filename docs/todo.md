# PHPantom — Roadmap

This document tracks planned work for PHPantom. Each item links to a
domain document with full context. Items are grouped into time-boxed
sprints (roughly 1-2 weeks each) and a backlog of ideas not yet
scheduled.

**Guiding priorities:** Completion accuracy → Type intelligence →
Cross-file navigation → Diagnostics → Code actions → Performance.

Items inside each sprint are ordered by priority (top = do first).
The backlog is ordered by impact (descending), then effort (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

# Scheduled Sprints

## Sprint 3 — Diagnostics quality (finishing touches)

| #   | Item                                                                                                                                                           | Impact | Effort |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------ |
| E0  | [Switch embedded stubs to master + LanguageLevelTypeAware](todo/external-stubs.md#e0-switch-embedded-stubs-to-master-and-apply-languageleveltypeaware-patches) | High   | Low    |

## Sprint 4 — Refactoring toolkit

| #   | Item                                                                               | Impact | Effort |
| --- | ---------------------------------------------------------------------------------- | ------ | ------ |
| R1  | [Extract cursor-context AST helper](todo/refactor.md#r1-cursor-context-ast-helper) | —      | Low    |
| A1  | [Extract method](todo/actions.md#a1-extract-method)                                | High   | High   |
| A2  | [Extract variable](todo/actions.md#a2-extract-variable)                            | Medium | Medium |
| A4  | [Inline variable](todo/actions.md#a4-inline-variable)                              | Medium | Medium |
| A6  | [Generate constructor](todo/actions.md#a6-generate-constructor)                    | Medium | Medium |
| A7  | [Promote constructor parameter](todo/actions.md#a7-promote-constructor-parameter)  | Medium | Low    |

## Sprint 5 — Polish for office adoption

| #   | Item                                                                          | Impact      | Effort |
| --- | ----------------------------------------------------------------------------- | ----------- | ------ |
|     | Clear [refactoring gate](todo/refactor.md)                                    | —           | —      |
| F1  | [Workspace symbol search](todo/lsp-features.md#f1-workspace-symbol-search)    | High        | Medium |
| F2  | [Document symbols / outline](todo/lsp-features.md#f2-document-symbols)        | High        | Low    |
| A8  | [Implement interface methods](todo/actions.md#a8-implement-interface-methods) | Medium-High | Medium |
| A9  | [Add missing use import (auto)](todo/actions.md#a9-auto-import)               | Medium-High | Medium |
| D1  | [Unknown class diagnostic](todo/diagnostics.md#d1-unknown-class-diagnostic)   | Medium      | Medium |
| D3  | [Unknown method / property diagnostic](todo/diagnostics.md#d3-unknown-member) | Medium      | Medium |
| D4  | [Unused variable warning](todo/diagnostics.md#d4-unused-variable)             | Medium      | Medium |

## Sprint 6 — Type intelligence depth

| #   | Item                                                                                                                       | Impact      | Effort |
| --- | -------------------------------------------------------------------------------------------------------------------------- | ----------- | ------ |
|     | Clear [refactoring gate](todo/refactor.md)                                                                                 | —           | —      |
| C2  | [`LanguageLevelTypeAware` version-aware type hints](todo/completion.md#c2-languageleveltypeaware-version-aware-type-hints) | Medium-High | Medium |
| C3  | [`#[ArrayShape]` return shapes on stub functions](todo/completion.md#c3-arrayshape-return-shapes-on-stub-functions)        | Medium      | Medium |
| T1  | [First-class callable resolution](todo/type-inference.md#t1-first-class-callables)                                         | Medium      | Medium |
| T2  | [`@phpstan-type` / `@psalm-type` local type aliases](todo/type-inference.md#t2-local-type-aliases)                         | Medium      | Medium |
| T3  | [`@phpstan-import-type` cross-file type imports](todo/type-inference.md#t3-cross-file-type-imports)                        | Medium      | Medium |

## Sprint 7 — Laravel excellence & stub accuracy

| #   | Item                                                                                          | Impact      | Effort      |
| --- | --------------------------------------------------------------------------------------------- | ----------- | ----------- |
|     | Clear [refactoring gate](todo/refactor.md)                                                    | —           | —           |
| L1  | [Eloquent model attribute completion](todo/laravel.md#l1-eloquent-model-attribute-completion) | High        | High        |
| L5  | [Blade component tag completion](todo/laravel.md#l5-blade-component-tags)                     | Medium      | Medium-High |
| E1  | [External stub packages (ide-helper, etc.)](todo/external-stubs.md#e1-external-stub-packages) | Medium-High | Medium      |
| E4  | [Stub version alignment with target PHP](todo/external-stubs.md#e4-stub-version-alignment)    | Medium      | Medium      |
| E5  | [Extension stub coverage audit](todo/external-stubs.md#e5-extension-stub-audit)               | Medium      | Low         |

## Sprint 8 — Blade support

| #   | Item                                             | Impact | Effort    |
| --- | ------------------------------------------------ | ------ | --------- |
|     | Clear [refactoring gate](todo/refactor.md)       | —      | —         |
| BL1 | [Blade template language support](todo/blade.md) | High   | Very High |

The Blade sprint is a placeholder. Scope will be refined after Sprint 7
ships. The goal is to provide a single-binary PHP + Blade language
server with Blade intelligence.

# Backlog

Items not yet assigned to a sprint. Worth doing eventually but
unlikely to move the needle for most users.

| #   | Item                                                                                                                                                         | Impact     | Effort      |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ----------- |
|     | **[Completion](todo/completion.md)**                                                                                                                         |            |             |
| C1  | Array functions needing new code paths                                                                                                                       | Medium     | High        |
| C4  | Go-to-definition for array shape keys via bracket access                                                                                                     | Low-Medium | Medium      |
| C5  | Non-array functions with dynamic return types                                                                                                                | Low        | High        |
| C6  | `#[ReturnTypeContract]` parameter-dependent return types                                                                                                     | Low        | Low         |
| C7  | `#[ExpectedValues]` parameter value suggestions                                                                                                              | Low        | Medium      |
| C8  | `class_alias()` support                                                                                                                                      | Low-Medium | Medium      |
|     | **[Type Inference](todo/type-inference.md)**                                                                                                                 |            |             |
| T4  | Non-empty-\* type narrowing and propagation                                                                                                                  | Low        | Low         |
| T5  | Fiber type resolution                                                                                                                                        | Low        | Low         |
| T6  | `Closure::bind()` / `Closure::fromCallable()` return type preservation                                                                                       | Low-Medium | Low-Medium  |
|     | **[Diagnostics](todo/diagnostics.md)**                                                                                                                       |            |             |
| D2  | Chain error propagation (flag only the first broken link)                                                                                                    | Medium     | Medium      |
| D5  | Diagnostic suppression intelligence                                                                                                                          | Medium     | Medium      |
| D10 | PHPMD diagnostic proxy                                                                                                                                       | Low        | Medium      |
|     | **[Code Actions](todo/actions.md)**                                                                                                                          |            |             |
| A3  | Switch → match conversion                                                                                                                                    | Low        | Medium      |
| A10 | Generate interface from class                                                                                                                                | Low-Medium | Medium      |
|     | **[LSP Features](todo/lsp-features.md)**                                                                                                                     |            |             |
| F3  | Incremental text sync                                                                                                                                        | Low-Medium | Medium      |
|     | **[Signature Help](todo/signature-help.md)**                                                                                                                 |            |             |
| S3  | Multiple overloaded signatures                                                                                                                               | Medium     | Medium-High |
| S4  | Named argument awareness in active parameter                                                                                                                 | Low-Medium | Medium      |
| S5  | Language construct signature help and hover                                                                                                                  | Low        | Low         |
|     | **[Laravel](todo/laravel.md)**                                                                                                                               |            |             |
| L4  | Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`)                                                                                            | Medium     | Medium      |
| L2  | `morphedByMany` missing from relationship method map                                                                                                         | Low-Medium | Low         |
| L3  | `$dates` array (deprecated)                                                                                                                                  | Low-Medium | Low         |
| L6  | Factory `has*`/`for*` relationship methods                                                                                                                   | Low-Medium | Medium      |
| L7  | `$pivot` property on BelongsToMany                                                                                                                           | Medium     | Medium-High |
| L8  | `withSum`/`withAvg`/`withMin`/`withMax` aggregate properties                                                                                                 | Low-Medium | Medium-High |
| L9  | Higher-order collection proxies                                                                                                                              | Low-Medium | Medium-High |
| L10 | `View::withX()` / `RedirectResponse::withX()` dynamic methods                                                                                                | Low        | Low         |
| L11 | `$appends` array                                                                                                                                             | Low        | Low         |
|     | **[External Stubs](todo/external-stubs.md)**                                                                                                                 |            |             |
| E2  | Project-level stubs as type resolution source                                                                                                                | Medium     | Medium      |
| E3  | IDE-provided and `.phpantom.toml` stub paths                                                                                                                 | Low-Medium | Low         |
| E6  | Stub install prompt for non-Composer projects                                                                                                                | Low        | Low         |
|     | **[Performance](todo/performance.md)**                                                                                                                       |            |             |
| P1a | `type_hint_to_classes` returns `Vec<Arc<ClassInfo>>`                                                                                                         | Low        | Low         |
| P1b | Propagate `Arc<ClassInfo>` through variable-resolution pipeline                                                                                              | Low        | Medium      |
| P2  | Type AST for `apply_substitution` (full refactor)                                                                                                            | Medium     | High        |
| P3  | Parallel pre-filter in `find_implementors`                                                                                                                   | Low-Medium | Medium      |
| P4  | `memmem` for block comment terminator search                                                                                                                 | Low        | Low         |
| P5  | `memmap2` for file reads during scanning                                                                                                                     | Low        | Low         |
| P6  | O(n²) transitive eviction in `evict_fqn`                                                                                                                     | Low        | Low         |
| P7  | `diag_pending_uris` uses `Vec::contains` for dedup                                                                                                           | Low        | Low         |
| P8  | `find_class_in_ast_map` linear fallback scan                                                                                                                 | Low        | Low         |
|     | **[Indexing](todo/indexing.md)**                                                                                                                             |            |             |
| X1  | Staleness detection and auto-refresh                                                                                                                         | Medium     | Medium      |
| X3  | Completion item detail on demand (`completionItem/resolve`)                                                                                                  | Medium     | Medium      |
| X2  | Parallel file processing — remaining work                                                                                                                    | Low-Medium | Medium      |
| X5  | Granular progress reporting for indexing, GTI, and Find References                                                                                           | Low-Medium | Medium      |
| X4  | Full background indexing (`strategy = "full"`)                                                                                                               | Medium     | High        |
| X6  | Disk cache (evaluate later)                                                                                                                                  | Medium     | High        |
|     | **[Bug Fixes](todo/bugs.md)**                                                                                                                                |            |             |
| B11 | [Diagnostic deduplication drops distinct diagnostics on same range](todo/bugs.md#b11--diagnostic-deduplication-drops-distinct-diagnostics-on-the-same-range) | Medium     | Low         |
| B12 | [PHPStan cache pruning uses length-only comparison](todo/bugs.md#b12--phpstan-cache-pruning-uses-length-only-comparison)                                     | Low        | Low         |
| B13 | [Argument count diagnostic flags too many arguments by default](todo/bugs.md#b13-argument-count-diagnostic-flags-too-many-arguments-by-default)              | High       | Low         |
|     | **[Inline Completion](todo/inline-completion.md)**                                                                                                           |            |             |
| N1  | Template engine (type-aware snippets)                                                                                                                        | Medium     | High        |
| N2  | N-gram prediction from PHP corpus                                                                                                                            | Medium     | Very High   |
| N3  | Fine-tuned GGUF sidecar model                                                                                                                                | Medium     | Very High   |
