<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Collection;

/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class ReviewCollection extends Collection
{
    /** @return array<TKey, TModel> */
    public function topRated(): array { return []; }

    /** @return float */
    public function averageRating(): float { return 0.0; }
}
