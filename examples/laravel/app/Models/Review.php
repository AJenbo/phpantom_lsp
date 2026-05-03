<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\CollectedBy;
use Illuminate\Database\Eloquent\Model;

/**
 * @template TKey of array-key
 * @template TModel
 * @extends \Illuminate\Database\Eloquent\Collection<TKey, TModel>
 */
class ReviewCollection extends \Illuminate\Database\Eloquent\Collection
{
    /** @return array<TKey, TModel> */
    public function topRated(): array { return []; }

    /** @return float */
    public function averageRating(): float { return 0.0; }
}

#[CollectedBy(ReviewCollection::class)]
class Review extends Model
{
    public function getTitle(): string { return ''; }
    public function getRating(): int { return 0; }

    /** @return \Illuminate\Database\Eloquent\Relations\HasMany<Review, $this> */
    public function replies(): mixed { return $this->hasMany(Review::class); }
}

enum OrderStatus: string
{
    case Pending = 'pending';
    case Processing = 'processing';
    case Completed = 'completed';
    case Cancelled = 'cancelled';

    public function label(): string { return $this->value; }
    public function isPending(): bool { return $this === self::Pending; }
}
