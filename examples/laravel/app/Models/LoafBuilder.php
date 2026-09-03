<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Builder;

/**
 * A custom builder written the way most projects write one: no
 * `@template`, no `@extends`. PHP has no generics, so the model the
 * builder queries is only known from the call site.
 */
class LoafBuilder extends Builder
{
    /**
     * @return $this
     */
    public function stale()
    {
        return $this->where('baked_at', '<', '-2 days');
    }
}
