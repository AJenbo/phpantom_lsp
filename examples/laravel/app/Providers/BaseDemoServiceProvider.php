<?php

namespace App\Providers;

use App\Support\PlainOven;
use Illuminate\Support\ServiceProvider;

/**
 * The base a package provider extends so its subclasses can register under
 * one shared container key without repeating the string.
 */
class BaseDemoServiceProvider extends ServiceProvider
{
    /**
     * The container key this package binds its services under.
     */
    public static $abstract = 'pastry.oven';

    public function register(): void
    {
        // The default this package ships.  DemoServiceProvider, which extends
        // this class, binds the same key to something else, and that is the
        // one the container ends up with.
        $this->app->singleton(static::$abstract, fn () => new PlainOven());
    }
}
