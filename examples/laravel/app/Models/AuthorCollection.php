<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Collection;

/**
 * @template TKey of array-key
 * @template TModel of BlogAuthor
 * @extends Collection<TKey, TModel>
 */
class AuthorCollection extends Collection
{
    /** @return static */
    public function active(): static { return $this->filter(fn($a) => $a->active); }

    /** @return array<string> */
    public function emails(): array { return $this->pluck('email')->all(); }

    /** @return static */
    public function byName(): static { return $this->sortBy('name'); }
}
