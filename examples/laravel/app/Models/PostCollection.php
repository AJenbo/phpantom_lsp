<?php

namespace App\Models;

/**
 * @template TKey of array-key
 * @template TModel of BlogPost
 * @extends \Illuminate\Database\Eloquent\Collection<TKey, TModel>
 */
class PostCollection extends \Illuminate\Database\Eloquent\Collection
{
    /** @return static */
    public function published(): static { return $this->filter(fn($p) => $p->published); }

    /** @return static */
    public function byNewest(): static { return $this->sortByDesc('created_at'); }

    /** @return array<string> */
    public function titles(): array { return $this->pluck('title')->all(); }
}
