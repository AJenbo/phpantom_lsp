<?php

namespace App\Facades;

use Illuminate\Support\Facades\Facade;

/**
 * A facade that names a container binding instead of a class, which is the
 * shape Laravel's own facades and most package facades use.
 *
 * Nothing here says what the members are: the key is bound in
 * DemoServiceProvider, so the class it stands for is only knowable from the
 * container.  PHPantom reads the binding and forwards that class's public
 * instance methods onto the facade.
 */
class PastryOven extends Facade
{
    protected static function getFacadeAccessor(): string
    {
        return 'pastry.oven';
    }
}
