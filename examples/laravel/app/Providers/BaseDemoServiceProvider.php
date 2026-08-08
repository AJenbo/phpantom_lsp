<?php

namespace App\Providers;

use Illuminate\Support\ServiceProvider;

/**
 * The base a package provider extends so its subclasses can register under
 * one shared container key without repeating the string.
 */
abstract class BaseDemoServiceProvider extends ServiceProvider
{
    /**
     * The container key this package binds its services under.
     */
    public static $abstract = 'pastry.oven';
}
