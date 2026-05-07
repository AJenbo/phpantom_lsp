<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\CollectedBy;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

#[CollectedBy(ReviewCollection::class)]
class Review extends Model
{
    public function getTitle(): string { return ''; }
    public function getRating(): int { return 0; }

    /** @return HasMany<Review, $this> */
    public function replies(): mixed { return $this->hasMany(Review::class); }
}
