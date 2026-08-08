<?php

namespace App\Providers;

use Illuminate\Support\Facades\Route;
use Illuminate\Support\ServiceProvider;

class RouteServiceProvider extends ServiceProvider
{
    public function boot(): void
    {
        // Routes registered with the fluent `Route::…->group(base_path(…))`
        // API instead of `$this->loadRoutesFrom(…)`.  PHPantom scans this
        // registration so `route('reviews.update')` resolves even though the
        // route file lives under app/Modules, not the conventional routes/ dir.
        Route::middleware('web')
            ->group(base_path('app/Modules/Reviews/routes.php'));

        // A router macro registers routes of its own.  `laravel/ui` ships
        // `Route::auth()` this way, binding the closure to the router
        // instance so `$this` inside it is the router, not this provider.
        // PHPantom walks the macro body wherever the macro is called, so
        // `route('login')` resolves and Ctrl+Click on it jumps to the
        // `->name('login')` below; `$this->get(...)` resolves and completes
        // as a `Router` method rather than being flagged as unknown on
        // `RouteServiceProvider`.
        Route::macro('bakeryAuth', function (): void {
            $this->get('login', fn () => view('auth.login'))->name('login');
            $this->post('logout', fn () => view('auth.login'))->name('logout');

            // A macro body may call another macro; both sets of names belong
            // to whichever route file called the outer one.
            $this->bakeryPasswordReset();
        });

        Route::macro('bakeryPasswordReset', function (): void {
            $this->get('password/reset', fn () => view('auth.login'))->name('password.request');
            $this->post('password/reset', fn () => view('auth.login'))->name('password.update');
        });
    }
}
