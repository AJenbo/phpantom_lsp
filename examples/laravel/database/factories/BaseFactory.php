<?php

namespace Database\Factories;

use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * Shared project factory base.
 *
 * The missing generic annotation is intentional: concrete factories exercise
 * PHPantom's convention inference through this intermediate parent.
 */
abstract class BaseFactory extends Factory
{
}
