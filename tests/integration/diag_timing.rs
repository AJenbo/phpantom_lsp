//! Quick timing test to measure diagnostic performance on `example.php`.
//!
//! Run with:
//!   cargo test --release -p phpantom_lsp --test diag_timing -- --nocapture
//!
//! The `warm_cache` tests simulate the real editing scenario: diagnostics
//! run once (cold, populates the resolved-class cache), then the user
//! edits a single file and diagnostics run again.  With targeted cache
//! invalidation, classes from unedited files stay cached and the second
//! pass is significantly faster.

use crate::common::{create_psr4_workspace, create_test_backend};
use std::time::Instant;

#[tokio::test]
async fn time_diagnostics_on_example_php() {
    let content = std::fs::read_to_string("example.php").expect("failed to read example.php");
    let uri = "file:///example.php";
    let backend = create_test_backend();
    backend.update_ast(uri, &content);

    // ── Deprecated diagnostics ──────────────────────────────────────────
    let start = Instant::now();
    let mut deprecated_out = Vec::new();
    backend.collect_deprecated_diagnostics(uri, &content, &mut deprecated_out);
    let deprecated_time = start.elapsed();

    // ── Unused import diagnostics ───────────────────────────────────────
    let start = Instant::now();
    let mut unused_out = Vec::new();
    backend.collect_unused_import_diagnostics(uri, &content, &mut unused_out);
    let unused_time = start.elapsed();

    // ── Unknown class diagnostics ───────────────────────────────────────
    let start = Instant::now();
    let mut unknown_out = Vec::new();
    backend.collect_unknown_class_diagnostics(uri, &content, &mut unknown_out);
    let unknown_time = start.elapsed();

    let total = deprecated_time + unused_time + unknown_time;

    eprintln!();
    eprintln!(
        "=== Diagnostic timing on example.php ({} lines) ===",
        content.lines().count()
    );
    eprintln!(
        "  deprecated:     {:>10.3?}  ({} diagnostics)",
        deprecated_time,
        deprecated_out.len()
    );
    eprintln!(
        "  unused_imports: {:>10.3?}  ({} diagnostics)",
        unused_time,
        unused_out.len()
    );
    eprintln!(
        "  unknown_classes:{:>10.3?}  ({} diagnostics)",
        unknown_time,
        unknown_out.len()
    );
    eprintln!("  ──────────────────────────────────");
    eprintln!("  TOTAL:          {:>10.3?}", total);
    eprintln!();

    // In debug builds the threshold is generous (20 s); in release builds
    // diagnostics on example.php should comfortably finish under 2 s.
    let budget_secs = if cfg!(debug_assertions) { 20.0 } else { 2.0 };
    assert!(
        total.as_secs_f64() < budget_secs,
        "Diagnostics took {:.3?} which exceeds the {:.0} s budget. \
         This runs on every keystroke — investigate which provider is slow.",
        total,
        budget_secs,
    );
}

/// Simulate the real editing loop: run diagnostics (cold), then re-parse
/// the same file (as `did_change` would) and run diagnostics again.
///
/// With targeted cache invalidation, only the classes defined in the
/// edited file are evicted.  Classes from vendor, stubs, and other user
/// files stay cached, making the second pass significantly faster.
#[tokio::test]
async fn time_diagnostics_warm_cache_example_php() {
    let content = std::fs::read_to_string("example.php").expect("failed to read example.php");
    let uri = "file:///example.php";
    let backend = create_test_backend();
    backend.update_ast(uri, &content);

    // ── Cold run: populates the resolved-class cache ────────────────────
    let start_cold = Instant::now();
    let mut out = Vec::new();
    backend.collect_deprecated_diagnostics(uri, &content, &mut out);
    backend.collect_unused_import_diagnostics(uri, &content, &mut out);
    backend.collect_unknown_class_diagnostics(uri, &content, &mut out);
    let cold_total = start_cold.elapsed();
    let cold_count = out.len();

    // ── Simulate an edit: re-parse the file (clears only this file's
    //    FQNs from the cache, not the entire cache) ──────────────────────
    backend.update_ast(uri, &content);

    // ── Warm run: unedited classes (vendor, stubs) are still cached ─────
    let start_warm = Instant::now();
    let mut out_warm = Vec::new();
    backend.collect_deprecated_diagnostics(uri, &content, &mut out_warm);
    backend.collect_unused_import_diagnostics(uri, &content, &mut out_warm);
    backend.collect_unknown_class_diagnostics(uri, &content, &mut out_warm);
    let warm_total = start_warm.elapsed();
    let warm_count = out_warm.len();

    eprintln!();
    eprintln!(
        "=== Warm-cache diagnostic timing on example.php ({} lines) ===",
        content.lines().count()
    );
    eprintln!(
        "  cold run:  {:>10.3?}  ({} diagnostics)",
        cold_total, cold_count
    );
    eprintln!(
        "  warm run:  {:>10.3?}  ({} diagnostics)",
        warm_total, warm_count
    );
    let speedup = cold_total.as_secs_f64() / warm_total.as_secs_f64().max(0.000001);
    eprintln!("  speedup:   {:.1}x", speedup);
    eprintln!();

    // The warm run should produce the same diagnostics as the cold run.
    assert_eq!(
        cold_count, warm_count,
        "warm run produced different diagnostic count ({} vs {})",
        warm_count, cold_count
    );
}

/// Cross-file warm-cache test with a real PSR-4 workspace.
///
/// This is the scenario that actually benefits from targeted invalidation:
/// vendor/framework classes live in separate files, the user edits only
/// their own file.  On the warm run, all vendor class resolutions stay
/// cached because `update_ast` only evicts FQNs defined in the edited file.
///
/// `example.php` puts everything in one file, so ALL FQNs get evicted on
/// every edit and the cache provides no cross-edit benefit.  This test
/// shows the real-world improvement.
#[tokio::test]
async fn time_diagnostics_warm_cache_cross_file() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/",
                "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
                "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
                "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/",
                "Illuminate\\Support\\": "vendor/illuminate/Support/"
            }
        }
    }"#;

    let model_php = r#"<?php
namespace Illuminate\Database\Eloquent;
/**
 * @method static \Illuminate\Database\Eloquent\Builder<static> where(string $column, mixed $operator = null, mixed $value = null)
 * @method static \Illuminate\Database\Eloquent\Builder<static> query()
 */
abstract class Model {
    /** @deprecated Use newQuery() instead */
    public static function on(string $connection = null): Builder { return new Builder(); }
}
"#;

    let builder_php = r#"<?php
namespace Illuminate\Database\Eloquent;
use Illuminate\Database\Concerns\BuildsQueries;
/**
 * @template TModel of \Illuminate\Database\Eloquent\Model
 * @mixin \Illuminate\Database\Query\Builder
 */
class Builder {
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;
    /** @return static */
    public function where(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
    /** @return static */
    public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
    /** @return \Illuminate\Database\Eloquent\Collection<int, TModel> */
    public function get(): Collection { return new Collection(); }
    /** @return static */
    public function limit(int $value): static { return $this; }
    /** @deprecated Use where() instead */
    public function whereRaw(string $sql): static { return $this; }
}
"#;

    let query_builder_php = r#"<?php
namespace Illuminate\Database\Query;
class Builder {
    /** @return static */
    public function whereIn(string $column, array $values): static { return $this; }
    /** @return static */
    public function groupBy(string ...$groups): static { return $this; }
    /** @deprecated Use whereIn() instead */
    public function whereInRaw(string $column, array $values): static { return $this; }
}
"#;

    let builds_queries_php = r#"<?php
namespace Illuminate\Database\Concerns;
/** @template TValue */
trait BuildsQueries {
    /** @return TValue|null */
    public function first(): mixed { return null; }
}
"#;

    let collection_php = r#"<?php
namespace Illuminate\Database\Eloquent;
/**
 * @template TKey of array-key
 * @template TModel
 */
class Collection {
    /** @return TModel|null */
    public function first(): mixed { return null; }
    /** @deprecated Use firstOrFail() instead */
    public function firstOr(): mixed { return null; }
}
"#;

    let support_collection_php = r#"<?php
namespace Illuminate\Support;
/**
 * @template TKey of array-key
 * @template TValue
 */
class Collection {
    /** @return TValue|null */
    public function first(): mixed { return null; }
    /** @return static */
    public function filter(callable $callback = null): static { return $this; }
}
"#;

    // User file: exercises many vendor class references.
    // Deliberately large to stress the resolved-class cache.  Each
    // `->method()` call triggers a `resolve_class_fully_cached` lookup,
    // so 50+ member accesses on vendor classes shows a clear speedup
    // when those resolutions survive across edits.
    let user_php = r#"<?php
namespace App;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Collection;

class Brand extends Model {
    public function scopeActive(Builder $query): void {}
    public function scopeOfGenre(Builder $query, string $genre): void {}
}

class Product extends Model {
    public function scopeInStock(Builder $query): void {}
}

class Category extends Model {
    public function scopeVisible(Builder $query): void {}
}

class UserService {
    public function brands(): void {
        $q1 = Brand::where('active', true);
        $q1->orderBy('name')->get();
        $q1->active();
        $q1->ofGenre('fiction');
        $q1->limit(10)->get();
        $q1->orderBy('created_at')->limit(5)->get();

        Brand::where('genre', 'fiction')->ofGenre('sci-fi')->get();
        Brand::where('active', 1)->orderBy('name')->first();
        Brand::where('active', 1)->orderBy('name')->limit(5)->get();
        Brand::where('x', 1)->where('y', 2)->where('z', 3)->get();
        Brand::where('a', 1)->active()->ofGenre('x')->orderBy('name')->get();
        Brand::where('b', 1)->limit(1)->first();
        Brand::where('c', 1)->get()->first();
    }

    public function products(): void {
        Product::where('in_stock', true)->inStock()->get();
        Product::where('price', '>', 100)->limit(10)->get();
        Product::where('active', true)->orderBy('price')->get();
        Product::where('active', true)->orderBy('price')->limit(20)->get();
        Product::where('active', true)->orderBy('name')->first();
        Product::where('active', true)->inStock()->orderBy('name')->get();
        Product::where('x', 1)->where('y', 2)->get();
        Product::where('x', 1)->where('y', 2)->limit(5)->get();
        Product::where('x', 1)->where('y', 2)->first();
        Product::where('x', 1)->where('y', 2)->orderBy('z')->get();

        $p = Product::where('active', true)->get();
        $p->first();
        $p2 = Product::where('price', '>', 50)->get();
        $p2->first();
        $p3 = Product::where('stock', '>', 0)->get();
        $p3->first();
    }

    public function categories(): void {
        Category::where('active', true)->visible()->get();
        Category::where('active', true)->orderBy('name')->get();
        Category::where('active', true)->orderBy('name')->limit(5)->get();
        Category::where('active', true)->orderBy('name')->first();
        Category::where('x', 1)->where('y', 2)->visible()->get();
        Category::where('x', 1)->where('y', 2)->orderBy('z')->get();
        Category::where('x', 1)->where('y', 2)->limit(10)->get();
        Category::where('x', 1)->visible()->orderBy('name')->limit(5)->get();

        $c = Category::where('active', true)->get();
        $c->first();
        $c2 = Category::where('parent_id', null)->get();
        $c2->first();
    }

    public function mixed(): void {
        $brands = Brand::where('active', true)->get();
        $brands->first();
        $products = Product::where('active', true)->get();
        $products->first();
        $categories = Category::where('active', true)->get();
        $categories->first();

        Brand::where('a', 1)->orderBy('b')->limit(5)->get()->first();
        Product::where('a', 1)->orderBy('b')->limit(5)->get()->first();
        Category::where('a', 1)->orderBy('b')->limit(5)->get()->first();

        Brand::where('x', 1)->get();
        Brand::where('x', 2)->get();
        Brand::where('x', 3)->get();
        Product::where('x', 1)->get();
        Product::where('x', 2)->get();
        Product::where('x', 3)->get();
        Category::where('x', 1)->get();
        Category::where('x', 2)->get();
        Category::where('x', 3)->get();
    }
}
"#;

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("vendor/illuminate/Eloquent/Model.php", model_php),
            ("vendor/illuminate/Eloquent/Builder.php", builder_php),
            ("vendor/illuminate/Eloquent/Collection.php", collection_php),
            ("vendor/illuminate/Query/Builder.php", query_builder_php),
            (
                "vendor/illuminate/Concerns/BuildsQueries.php",
                builds_queries_php,
            ),
            (
                "vendor/illuminate/Support/Collection.php",
                support_collection_php,
            ),
            ("src/UserService.php", user_php),
        ],
    );

    let user_uri = format!(
        "file://{}",
        _dir.path().join("src/UserService.php").display()
    );
    backend.update_ast(&user_uri, user_php);

    // ── Cold run: populates the resolved-class cache ────────────────────
    let start_cold = Instant::now();
    let mut out_cold = Vec::new();
    backend.collect_deprecated_diagnostics(&user_uri, user_php, &mut out_cold);
    backend.collect_unused_import_diagnostics(&user_uri, user_php, &mut out_cold);
    backend.collect_unknown_class_diagnostics(&user_uri, user_php, &mut out_cold);
    let cold_total = start_cold.elapsed();
    let cold_count = out_cold.len();

    // ── Simulate editing the user file only ─────────────────────────────
    // This evicts App\Brand, App\Product, App\UserService from the cache
    // but leaves all Illuminate\* entries intact.
    backend.update_ast(&user_uri, user_php);

    // ── Warm run: vendor classes are still cached ───────────────────────
    let start_warm = Instant::now();
    let mut out_warm = Vec::new();
    backend.collect_deprecated_diagnostics(&user_uri, user_php, &mut out_warm);
    backend.collect_unused_import_diagnostics(&user_uri, user_php, &mut out_warm);
    backend.collect_unknown_class_diagnostics(&user_uri, user_php, &mut out_warm);
    let warm_total = start_warm.elapsed();
    let warm_count = out_warm.len();

    eprintln!();
    eprintln!("=== Cross-file warm-cache diagnostic timing ===");
    eprintln!(
        "  cold run:  {:>10.3?}  ({} diagnostics)",
        cold_total, cold_count
    );
    eprintln!(
        "  warm run:  {:>10.3?}  ({} diagnostics)",
        warm_total, warm_count
    );
    let speedup = cold_total.as_secs_f64() / warm_total.as_secs_f64().max(0.000001);
    eprintln!("  speedup:   {:.1}x", speedup);
    eprintln!();

    assert_eq!(
        cold_count, warm_count,
        "warm run produced different diagnostic count ({} vs {})",
        warm_count, cold_count
    );
}

#[tokio::test]
async fn time_diagnostics_on_phpstan_fixture() {
    let path = "benches/fixtures/diagnostics/phpstan.php";
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping: {path} not found");
            return;
        }
    };
    let uri = "file:///bench/phpstan.php";
    let backend = create_test_backend();
    backend.update_ast(uri, &content);

    let start = Instant::now();
    let mut deprecated_out = Vec::new();
    backend.collect_deprecated_diagnostics(uri, &content, &mut deprecated_out);
    let deprecated_time = start.elapsed();

    let start = Instant::now();
    let mut unused_out = Vec::new();
    backend.collect_unused_import_diagnostics(uri, &content, &mut unused_out);
    let unused_time = start.elapsed();

    let start = Instant::now();
    let mut unknown_out = Vec::new();
    backend.collect_unknown_class_diagnostics(uri, &content, &mut unknown_out);
    let unknown_time = start.elapsed();

    let total = deprecated_time + unused_time + unknown_time;

    eprintln!();
    eprintln!(
        "=== Diagnostic timing on phpstan.php ({} lines) ===",
        content.lines().count()
    );
    eprintln!(
        "  deprecated:     {:>10.3?}  ({} diagnostics)",
        deprecated_time,
        deprecated_out.len()
    );
    eprintln!(
        "  unused_imports: {:>10.3?}  ({} diagnostics)",
        unused_time,
        unused_out.len()
    );
    eprintln!(
        "  unknown_classes:{:>10.3?}  ({} diagnostics)",
        unknown_time,
        unknown_out.len()
    );
    eprintln!("  ──────────────────────────────────");
    eprintln!("  TOTAL:          {:>10.3?}", total);
    eprintln!();

    let budget_secs = if cfg!(debug_assertions) { 120.0 } else { 5.0 };
    assert!(
        total.as_secs_f64() < budget_secs,
        "Diagnostics took {:.3?} on the large phpstan fixture — too slow for interactive use \
         (budget: {:.0} s).",
        total,
        budget_secs,
    );
}

/// Warm-cache test on the phpstan fixture (larger file, more class references).
#[tokio::test]
async fn time_diagnostics_warm_cache_phpstan() {
    let path = "benches/fixtures/diagnostics/phpstan.php";
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping: {path} not found");
            return;
        }
    };
    let uri = "file:///bench/phpstan.php";
    let backend = create_test_backend();
    backend.update_ast(uri, &content);

    // ── Cold run ────────────────────────────────────────────────────────
    let start_cold = Instant::now();
    let mut out = Vec::new();
    backend.collect_deprecated_diagnostics(uri, &content, &mut out);
    backend.collect_unused_import_diagnostics(uri, &content, &mut out);
    backend.collect_unknown_class_diagnostics(uri, &content, &mut out);
    let cold_total = start_cold.elapsed();
    let cold_count = out.len();

    // ── Simulate edit ───────────────────────────────────────────────────
    backend.update_ast(uri, &content);

    // ── Warm run ────────────────────────────────────────────────────────
    let start_warm = Instant::now();
    let mut out_warm = Vec::new();
    backend.collect_deprecated_diagnostics(uri, &content, &mut out_warm);
    backend.collect_unused_import_diagnostics(uri, &content, &mut out_warm);
    backend.collect_unknown_class_diagnostics(uri, &content, &mut out_warm);
    let warm_total = start_warm.elapsed();
    let warm_count = out_warm.len();

    eprintln!();
    eprintln!(
        "=== Warm-cache diagnostic timing on phpstan.php ({} lines) ===",
        content.lines().count()
    );
    eprintln!(
        "  cold run:  {:>10.3?}  ({} diagnostics)",
        cold_total, cold_count
    );
    eprintln!(
        "  warm run:  {:>10.3?}  ({} diagnostics)",
        warm_total, warm_count
    );
    let speedup = cold_total.as_secs_f64() / warm_total.as_secs_f64().max(0.000001);
    eprintln!("  speedup:   {:.1}x", speedup);
    eprintln!();

    assert_eq!(
        cold_count, warm_count,
        "warm run produced different diagnostic count ({} vs {})",
        warm_count, cold_count
    );
}
