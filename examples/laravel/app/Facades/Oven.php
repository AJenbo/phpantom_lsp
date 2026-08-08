<?php

namespace App\Facades;

use App\Support\BakeryService;
use Illuminate\Support\Facades\Facade;

/**
 * An app-defined facade with no generated `@method static` docblock, which
 * is what a project that never ran `facade-documenter` ends up with.
 *
 * `Facade::__callStatic()` forwards to the class the accessor names, so
 * every public instance method of BakeryService is callable statically
 * here even though nothing lists them.
 */
class Oven extends Facade
{
    protected static function getFacadeAccessor(): string
    {
        return BakeryService::class;
    }
}
