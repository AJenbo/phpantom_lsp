<?php

namespace App\Providers;

use App\Models\Bakery;
use App\Models\BlogPost;
use App\Support\BakeryService;
use App\Support\CarbonMixin;
use App\Support\CollectionMixin;
use App\Support\CroissantSupplier;
use Carbon\CarbonImmutable;
use Illuminate\Database\Eloquent\Relations\Relation;
use Illuminate\Support\Collection;

class DemoServiceProvider extends BaseDemoServiceProvider
{
    public function register(): void
    {
        // A container key is not always written as a literal.  This one lives
        // in a static property on the base provider, so `static::$abstract`
        // is all the subclass writes.  PHPantom folds the property against
        // this concrete provider class, which is where the parent chain is
        // known, so `app('demo.bakery')` resolves to BakeryService.
        $this->app->singleton(static::$abstract, fn () => new BakeryService());

        // `alias()` takes its arguments the other way round from `bind()`:
        // the second one is the new name, the first is what it stands for.
        $this->app->alias(CroissantSupplier::class, static::$abstract . '.supplier');
    }

    public function boot(): void
    {
        // The morph map replaces the model FQCN with a short alias in every
        // polymorphic `*_type` column.  PHPantom reads the registration from
        // here, so the alias strings elsewhere in the project hover with the
        // model they stand for and go-to-definition jumps to it.
        Relation::morphMap([
            'blog_post' => BlogPost::class,
            'bakery'    => Bakery::class,
        ]);

        // A macro registered here becomes a real method on Collection:
        // it autocompletes, hovers with this signature, and type-checks.
        Collection::macro('sumField', function (string $field): float {
            return $this->sum($field);
        });

        // A mixin registers one macro per public method of the given object,
        // each taking the signature of the closure that method returns.
        // PHPantom recovers those from CollectionMixin's source.
        Collection::mixin(new CollectionMixin());

        // Carbon supports the same `macro()` pattern as Laravel's Macroable.
        // The closure is bound with the target as scope, so `self::`/`static::`
        // refer to CarbonImmutable and protected helpers like `self::this()`
        // (the instance the macro is called on) resolve:
        CarbonImmutable::macro('diffFromYear', function (int $year, bool $absolute = false): string {
            return self::this()->diffForHumans(
                CarbonImmutable::create($year, 1, 1),
                ['syntax' => \Carbon\CarbonInterface::DIFF_ABSOLUTE]
            );
        });

        // Carbon also supports trait-based mixins (since Carbon 2.23.0):
        // each public method of the trait becomes a method on the target,
        // using the trait method's own signature directly.
        CarbonImmutable::mixin(CarbonMixin::class);
    }
}
