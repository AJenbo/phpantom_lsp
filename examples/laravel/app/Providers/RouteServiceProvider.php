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
        // `Route::auth()` this way.  PHPantom walks the macro body wherever
        // the macro is called, so `route('login')` resolves and Ctrl+Click on
        // it jumps to the `->name('login')` below.
        Route::macro('bakeryAuth', function (): void {
            Route::get('login', fn () => view('welcome'))->name('login');
            Route::post('logout', fn () => view('welcome'))->name('logout');

            // A macro body may call another macro; both sets of names belong
            // to whichever route file called the outer one.
            Route::bakeryPasswordReset();
        });

        Route::macro('bakeryPasswordReset', function (): void {
            Route::get('password/reset', fn () => view('welcome'))->name('password.request');
            Route::post('password/reset', fn () => view('welcome'))->name('password.update');
        });
    }
}
