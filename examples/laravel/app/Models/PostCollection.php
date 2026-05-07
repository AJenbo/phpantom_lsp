<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Collection;

/**
 * @template TKey of array-key
 * @template TModel of BlogPost
 * @extends Collection<TKey, TModel>
 */
class PostCollection extends Collection
{
    /** @return static */
    public function published(): static { return $this->filter(fn($p) => $p->published); }

    /** @return static */
    public function byNewest(): static { return $this->sortByDesc('created_at'); }

    /** @return array<string> */
    public function titles(): array { return $this->pluck('title')->all(); }
}
