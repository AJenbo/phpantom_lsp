<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\UseEloquentBuilder;
use Illuminate\Database\Eloquent\Model;

#[UseEloquentBuilder(LoafBuilder::class)]
class Loaf extends Model
{
    public function getWeight(): int { return 0; }
}
