<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\CollectedBy;
use Illuminate\Database\Eloquent\Model;

#[CollectedBy(AuthorCollection::class)]

/**
 * @method static \Illuminate\Database\Eloquent\Builder<static> withTrashed(bool $withTrashed = true)
 * @method static \Illuminate\Database\Eloquent\Builder<static> onlyTrashed()
 */
class BlogAuthor extends Model
{
    protected $fillable = ['name', 'email', 'genre'];

    protected $guarded = ['id'];

    protected $hidden = ['password'];

    /** @return \Illuminate\Database\Eloquent\Relations\HasMany<BlogPost, $this> */
    public function posts(): mixed { return $this->hasMany(BlogPost::class); }

    /** @return \Illuminate\Database\Eloquent\Relations\HasOne<AuthorProfile, $this> */
    public function profile(): mixed { return $this->hasOne(AuthorProfile::class); }

    public function scopeActive(\Illuminate\Database\Eloquent\Builder $query): void
    {
        $query->where('active', true);
    }

    public function scopeOfGenre(\Illuminate\Database\Eloquent\Builder $query, string $genre): void
    {
        $query->where('genre', $genre);
    }
}
