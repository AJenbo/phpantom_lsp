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

## Sprint 4 — Refactoring toolkit & type inference

| #   | Item                                                                                                                                                                       | Impact | Effort |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------ |
| L13 | [`where{PropertyName}()` dynamic methods on Builder](todo/laravel.md#l13-wherepropertyname-dynamic-methods-on-builder)                                                     | High   | Medium |
| H17 | [`missingType.iterableValue` — add `@return` with inferred element type](todo/phpstan-actions.md#h17-missingtype-iterablevalue-return-type--add-return-with-iterable-type) | Medium | High   |
| H10 | [`return.unusedType` — remove unused type from return union](todo/phpstan-actions.md#h10-returnunusedtype--remove-unused-type-from-return-union)                           | Medium | Medium |
| H6  | `return.type` — update return type to match actual returns                                                                                                                 | Medium | Medium |
|     | **Release 0.7.0**                                                                                                                                                          |        |        |

## Sprint 5 — Polish for office adoption

| #   | Item                                                                                             | Impact | Effort |
| --- | ------------------------------------------------------------------------------------------------ | ------ | ------ |
|     | Clear [refactoring gate](todo/refactor.md)                                                       | —      | —      |
| D8  | [Undefined variable diagnostic](todo/diagnostics.md#d8-undefined-variable-diagnostic)            | High   | Medium |
| D11 | [Invalid class-like kind in context](todo/diagnostics.md#d11-invalid-class-like-kind-in-context) | Medium | Low    |
|     | **Release 0.8.0**                                                                                |        |        |

> **Note:** F1 (Workspace symbol search), F2 (Document symbols), A8
> (Implement interface methods), A9 (Auto import), D1 (Unknown class
> diagnostic), and D3 (Unknown member diagnostic) were originally
> planned here but have already shipped.

## Sprint 6 — Type intelligence depth

| #   | Item                                                                                                                | Impact | Effort |
| --- | ------------------------------------------------------------------------------------------------------------------- | ------ | ------ |
|     | Clear [refactoring gate](todo/refactor.md)                                                                          | —      | —      |
| C2  | [`#[ArrayShape]` return shapes on stub functions](todo/completion.md#c2-arrayshape-return-shapes-on-stub-functions) | Medium | Medium |
| T7  | [`key-of<T>` and `value-of<T>` resolution](todo/type-inference.md#t7-key-oft-and-value-oft-resolution)              | Medium | Medium |

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

| #    | Item                                                                                                                                                                        | Impact      | Effort         |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- | -------------- |
|      | **[Completion](todo/completion.md)**                                                                                                                                        |             |                |
| C1   | Array functions needing new code paths                                                                                                                                      | Medium      | High           |
| C9   | [Lazy documentation via `completionItem/resolve`](todo/completion.md#c9-lazy-documentation-via-completionitemresolve)                                                       | Medium      | Medium         |
| C11  | [Smarter member ordering after `->` / `::`](todo/completion.md#c11-smarter-member-ordering-after----)                                                                       | Medium      | Medium         |
| C3   | Go-to-definition for array shape keys via bracket access                                                                                                                    | Low-Medium  | Medium         |
| C7   | `class_alias()` support                                                                                                                                                     | Low-Medium  | Medium         |
| C4   | Non-array functions with dynamic return types                                                                                                                               | Low         | High           |
| C5   | `#[ReturnTypeContract]` parameter-dependent return types                                                                                                                    | Low         | Low            |
| C6   | `#[ExpectedValues]` parameter value suggestions                                                                                                                             | Low         | Medium         |
| C10  | [Deprecation markers on class-name completions from all sources](todo/completion.md#c10-deprecation-markers-on-class-name-completions-from-all-sources)                     | Low         | Low            |
|      | **[Type Inference](todo/type-inference.md)**                                                                                                                                |             |                |
| T19  | [Structured type representation](todo/type-inference.md#t19-structured-type-representation) (replace string-based types with `PhpType` enum)                                | High        | Very High      |
| T20  | [Type narrowing reconciliation engine](todo/type-inference.md#t20-type-narrowing-reconciliation-engine) (sure/sureNot tracking, AND/OR algebra)                             | Medium-High | High           |
| T21  | [Bidirectional template inference](todo/type-inference.md#t21-bidirectional-template-inference-upperlower-bounds) (upper/lower bounds, variance)                            | Medium      | Medium-High    |
| T6   | `Closure::bind()` / `Closure::fromCallable()` return type preservation                                                                                                      | Low-Medium  | Low-Medium     |
| T12  | [Intersection types flattened to unions by `type_strings_joined`](todo/type-inference.md#t12-intersection-types-flattened-to-unions-by-type_strings_joined)                 | Low-Medium  | Low (after M4) |
| T13  | [Closure variables lose callable signature detail](todo/type-inference.md#t13-closure-variables-lose-callable-signature-detail)                                             | Low-Medium  | Medium         |
| T4   | Non-empty-\* type narrowing and propagation                                                                                                                                 | Low         | Low            |
| T5   | Fiber type resolution                                                                                                                                                       | Low         | Low            |
| T9   | [Dead-code elimination after `never`-returning calls](todo/type-inference.md#t9-dead-code-elimination-after-never-returning-calls)                                          | Low         | Low-Medium     |
| T10  | [Ternary expression as RHS of list destructuring](todo/type-inference.md#t10-ternary-expression-as-rhs-of-list-destructuring)                                               | Low         | Low-Medium     |
| T11  | [Nested list destructuring](todo/type-inference.md#t11-nested-list-destructuring)                                                                                           | Low         | Low-Medium     |
|      | **[Diagnostics](todo/diagnostics.md)**                                                                                                                                      |             |                |
| D5   | Diagnostic suppression intelligence                                                                                                                                         | Medium      | Medium         |
| D12  | [Mago linter integration](todo/diagnostics.md#d12-mago-linter-integration-optional-diagnostics) (optional, ~159 AST-only lint rules)                                        | Medium      | Medium         |
| D6   | [Unreachable code diagnostic](todo/diagnostics.md#d6-unreachable-code-diagnostic)                                                                                           | Low-Medium  | Low            |
| D10  | PHPMD diagnostic proxy                                                                                                                                                      | Low         | Medium         |
|      | **[Bug Fixes](todo/bugs.md)**                                                                                                                                               |             |                |
|      | **[Code Actions](todo/actions.md)**                                                                                                                                         |             |                |
| A8   | [Update docblock to match signature](todo/actions.md#a8-update-docblock-to-match-signature)                                                                                 | Medium      | Medium         |
| A16  | [Snippet placeholder for extracted method name](todo/actions.md#a16-snippet-placeholder-for-extracted-method-name) (lets the user type over the generated name immediately) | Medium      | Low-Medium     |
| A25  | [`strpos` → `str_contains`](todo/actions.md#a25-strpos--str_contains-php-80) (PHP 8.0+)                                                                                     | Medium      | Low            |
| A28  | [Explicit nullable parameter type](todo/actions.md#a28-explicit-nullable-parameter-type-php-84-deprecation) (PHP 8.4 deprecation)                                           | Medium      | Low            |
| A10  | [Generate interface from class](todo/actions.md#a10-generate-interface-from-class)                                                                                          | Low-Medium  | Medium         |
| A29  | [Simplify boolean return](todo/actions.md#a29-simplify-boolean-return) (`if (cond) return true; return false;` → `return cond;`)                                            | Low-Medium  | Medium         |
| A31  | [Remove always-else](todo/actions.md#a31-remove-always-else-extract-guard-clause) (extract guard clause)                                                                    | Low-Medium  | Medium         |
| A34  | [Unified code action handler architecture](todo/actions.md#a34-unified-code-action-handler-architecture) (closure-based resolve, unified fix type)                          | Medium      | Medium-High    |
| A3   | Switch → match conversion                                                                                                                                                   | Low         | Medium         |
|      | **[PHPStan Code Actions](todo/phpstan-actions.md)**                                                                                                                         |             |                |
| H4   | `assign.byRefForeachExpr` — unset by-reference foreach variable                                                                                                             | Medium      | Medium         |
| H13  | `property.notFound` — declare missing property (same-class)                                                                                                                 | Medium      | Medium         |
| H15  | Template bound from tip — add `@template T of X`                                                                                                                            | Medium      | Medium         |
| H16  | `match.unhandled` — add missing match arms                                                                                                                                  | Medium      | Medium         |
| H17  | `missingType.iterableValue` — add `@return` with inferred element type                                                                                                      | Medium      | High           |
| H19  | `property.unused` / `method.unused` — remove unused member                                                                                                                  | Low         | Low            |
| H20  | `generics.callSiteVarianceRedundant` — remove redundant variance annotation                                                                                                 | Low         | Low            |
| H23  | `instanceof.alwaysTrue` — remove redundant instanceof check                                                                                                                 | Low         | Low            |
| H24  | `catch.neverThrown` — remove unnecessary catch clause                                                                                                                       | Low         | Low            |
|      | **[CLI Fix Rules](todo/fix-cli.md)**                                                                                                                                        |             |                |
| FX1  | [`deprecated` — replace deprecated symbol usage](todo/fix-cli.md#fx1-deprecated--replace-deprecated-symbol-usage)                                                           | Medium      | Medium         |
| FX2  | [`unused_variable` — remove unused variables](todo/fix-cli.md#fx2-unused_variable--remove-unused-variables)                                                                 | Medium      | Medium         |
| FX3  | [`phpstan.return.unusedType` — remove unused type from return union](todo/fix-cli.md#fx3-phpstanreturnunusedtype--remove-unused-type-from-return-union)                     | Medium      | Medium         |
| FX4  | [`phpstan.missingType.iterableValue` — add `@return` with iterable type](todo/fix-cli.md#fx4-phpstanmissingtypeiterablevalue--add-return-with-iterable-type)                | Medium      | Medium         |
| FX5  | [`phpstan.property.unused` / `phpstan.method.unused` — remove unused member](todo/fix-cli.md#fx5-phpstanpropertyunused--phpstanmethodunused--remove-unused-member)          | Low         | Low            |
| FX6  | [`phpstan.generics.callSiteVarianceRedundant` — remove redundant variance](todo/fix-cli.md#fx6-phpstangenericscallsitevarianceredundant--remove-redundant-variance)         | Low         | Low            |
|      | **[LSP Features](todo/lsp-features.md)**                                                                                                                                    |             |                |
| F2   | [Partial result streaming via `$/progress`](todo/lsp-features.md#f2-partial-result-streaming-via-progress)                                                                  | Medium      | Medium-High    |
| F3   | Incremental text sync                                                                                                                                                       | Low-Medium  | Medium         |
|      | **[Signature Help](todo/signature-help.md)**                                                                                                                                |             |                |
| S1   | [Attribute constructor signature help](todo/signature-help.md#s1-attribute-constructor-signature-help)                                                                      | Medium      | Medium         |
| S2   | [Closure / arrow function parameter signature help](todo/signature-help.md#s2-closure--arrow-function-parameter-signature-help)                                             | Medium      | Medium         |
| S3   | Multiple overloaded signatures                                                                                                                                              | Medium      | Medium-High    |
| S4   | Named argument awareness in active parameter                                                                                                                                | Low-Medium  | Medium         |
| S5   | Language construct signature help and hover                                                                                                                                 | Low         | Low            |
|      | **[Laravel](todo/laravel.md)**                                                                                                                                              |             |                |
| L4   | Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`)                                                                                                           | Medium      | Medium         |
| L2   | [`morphedByMany` missing from relationship method map](todo/laravel.md#l2-morphedbymany-missing-from-relationship-method-map)                                               | Low-Medium  | Low            |
| L3   | `$dates` array (deprecated)                                                                                                                                                 | Low-Medium  | Low            |
| L6   | Factory `has*`/`for*` relationship methods                                                                                                                                  | Low-Medium  | Medium         |
| L7   | `$pivot` property on BelongsToMany                                                                                                                                          | Medium      | Medium-High    |
| L8   | `withSum`/`withAvg`/`withMin`/`withMax` aggregate properties                                                                                                                | Low-Medium  | Medium-High    |
| L9   | Higher-order collection proxies                                                                                                                                             | Low-Medium  | Medium-High    |
| L10  | `View::withX()` / `RedirectResponse::withX()` dynamic methods                                                                                                               | Low         | Low            |
|      | **[External Stubs](todo/external-stubs.md)**                                                                                                                                |             |                |
| E2   | Project-level stubs as type resolution source                                                                                                                               | Medium      | Medium         |
| E3   | IDE-provided and `.phpantom.toml` stub paths                                                                                                                                | Low-Medium  | Low            |
| E6   | Stub install prompt for non-Composer projects                                                                                                                               | Low         | Low            |
|      | **[Performance](todo/performance.md)**                                                                                                                                      |             |                |
| P1.5 | [Layered class resolution (zero-copy inheritance)](todo/performance.md#p15-layered-class-resolution-zero-copy-inheritance)                                                  | High        | Very High      |
| P13  | [Tiered storage: drop per-file maps for non-open files](todo/performance.md#p13-tiered-storage-drop-per-file-maps-for-non-open-files)                                       | Medium-High | Medium-High    |
| P9   | [`resolved_class_cache` generic-arg specialisation](todo/performance.md#p9-resolved_class_cache-generic-arg-specialisation)                                                 | Medium      | Medium         |
| P14  | [Eager docblock parsing into structured fields](todo/performance.md#p14-eager-docblock-parsing-into-structured-fields)                                                      | Medium      | Medium         |
| P10  | [Redundant `parse_and_cache_file` from multiple threads](todo/performance.md#p10-redundant-parse_and_cache_file-from-multiple-threads)                                      | Medium      | Low            |
| P11  | [Uncached base-resolution in `build_scope_methods_for_builder`](todo/performance.md#p11-uncached-base-resolution-in-build_scope_methods_for_builder)                        | Low-Medium  | Low            |
| P3   | Parallel pre-filter in `find_implementors`                                                                                                                                  | Low-Medium  | Medium         |
| P1a  | `type_hint_to_classes` returns `Vec<Arc<ClassInfo>>`                                                                                                                        | Low         | Low            |
| P1b  | Propagate `Arc<ClassInfo>` through variable-resolution pipeline                                                                                                             | Low         | Medium         |
| P4   | `memmem` for block comment terminator search                                                                                                                                | Low         | Low            |
| P5   | `memmap2` for file reads during scanning                                                                                                                                    | Low         | Low            |
| P6   | O(n²) transitive eviction in `evict_fqn`                                                                                                                                    | Low         | Low            |
| P7   | `diag_pending_uris` uses `Vec::contains` for dedup                                                                                                                          | Low         | Low            |
| P8   | `find_class_in_ast_map` linear fallback scan                                                                                                                                | Low         | Low            |
| P12  | [`find_or_load_function` Phase 1.75 serial bottleneck](todo/performance.md#p12-find_or_load_function-phase-175-serial-bottleneck)                                           | Low         | Low            |
| P17  | [`mago-names` resolution on the parse hot path](todo/performance.md#p17-mago-names-resolution-on-the-parse-hot-path)                                                        | Medium      | Low            |
| P18  | [Subtype result caching](todo/performance.md#p18-subtype-result-caching) (per-request HashMap for hierarchy walks)                                                          | Medium      | Low            |
|      | **[Indexing](todo/indexing.md)**                                                                                                                                            |             |                |
| X1   | Staleness detection and auto-refresh                                                                                                                                        | Medium      | Medium         |
| X3   | Completion item detail on demand (`completionItem/resolve`)                                                                                                                 | Medium      | Medium         |
| X7   | [Recency tracking](todo/indexing.md#x7-recency-tracking)                                                                                                                    | Medium      | Medium         |
| X2   | Parallel file processing — remaining work                                                                                                                                   | Low-Medium  | Medium         |
| X5   | Granular progress reporting for indexing, GTI, and Find References                                                                                                          | Low-Medium  | Medium         |
| X4   | Full background indexing (`strategy = "full"`)                                                                                                                              | Medium      | High           |
| X6   | Disk cache (evaluate later)                                                                                                                                                 | Medium      | High           |
|      | **[Inline Completion](todo/inline-completion.md)**                                                                                                                          |             |                |
| N1   | Template engine (type-aware snippets)                                                                                                                                       | Medium      | High           |
| N2   | N-gram prediction from PHP corpus                                                                                                                                           | Medium      | Very High      |
| N3   | Fine-tuned GGUF sidecar model                                                                                                                                               | Medium      | Very High      |
|      | **[Phpactor Test Parity](todo/phpactor-test-parity.md)**                                                                                                                    |             |                |
| Q1   | `is_callable()` / `is_float()` narrowing fixtures                                                                                                                           | Low         | Low            |
| Q2   | `in_array()` with class constants in haystack                                                                                                                               | Low         | Low            |
| Q3   | Elseif with terminating statement (`die`/`throw`)                                                                                                                           | Low         | Low            |
| Q4   | Else-branch assignment union merging                                                                                                                                        | Low         | Low            |
| Q5   | Combined negated type guards (`!is_string && !instanceof`)                                                                                                                  | Low         | Low            |
| Q6   | Namespace-qualified instanceof in or-chains                                                                                                                                 | Low         | Low            |
| Q7   | Ternary assignment producing union type                                                                                                                                     | Low         | Low            |
| Q8   | Cast expression type resolution (`(string)`, `(int)`, …)                                                                                                                    | Low         | Low            |
| Q9   | Variadic parameter type inside function body                                                                                                                                | Low         | Low            |
| Q10  | `list<T>` type alias resolution                                                                                                                                             | Low         | Low            |
| Q11  | `string\|false` return type                                                                                                                                                 | Low         | Low            |
| Q12  | Union from relative docblock class names                                                                                                                                    | Low         | Low            |
| Q13  | Callable / Closure docblock param types                                                                                                                                     | Low         | Low-Medium     |
| Q14  | `int<min,max>` range types                                                                                                                                                  | Low         | Low-Medium     |
| Q15  | Parenthesized union types with narrowing                                                                                                                                    | Low         | Low-Medium     |
| Q16  | Variable-variable `${$bar}` resolution                                                                                                                                      | Low         | Low-Medium     |
| Q17  | `global` keyword variable access                                                                                                                                            | Low         | Low-Medium     |
| Q19  | Array mutation tracking (`$arr[] = …`)                                                                                                                                      | Low         | Medium         |
| Q20  | Return statement body type inference                                                                                                                                        | Low         | Medium         |
| Q21  | Binary expression type inference (arithmetic, concat, …)                                                                                                                    | Low         | High           |
| Q22  | Postfix increment / decrement literal types                                                                                                                                 | Low         | Low            |
