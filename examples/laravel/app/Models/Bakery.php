<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\Scope;
use Illuminate\Database\Eloquent\Casts\Attribute;
use Illuminate\Database\Eloquent\Model;

class Bakery extends Model
{
    protected $fillable = ['flour'];

    protected $guarded = ['kitchen_id'];

    protected $hidden = ['oven_code'];

    protected $dates = ['defrosted_at'];

    protected $visible = ['rye_blend'];

    protected $appends = ['warmth'];

    protected $casts = [
        'apricot'    => 'boolean',
        'dough_temp' => 'float',
        'icing'      => FrostingCast::class,
        'jam_flavor' => JamFlavor::class,
        'notes'      => 'array',
        'proved_at'  => 'datetime',
    ];

    protected function casts(): array
    {
        return [
            'quality' => 'float',
        ];
    }

    protected $attributes = [
        'croissant'   => 'plain',
        'egg_count'   => 0,
        'gluten_free' => false,
    ];

    /** @return \Illuminate\Database\Eloquent\Relations\HasMany<Loaf, $this> */
    public function baguettes(): mixed { return $this->hasMany(Loaf::class); }

    /** @return \Illuminate\Database\Eloquent\Relations\HasOne<Baker, $this> */
    public function headBaker(): mixed { return $this->hasOne(Baker::class); }

    /** @return \Illuminate\Database\Eloquent\Relations\BelongsToMany<BakeryRecipe, $this> */
    public function masterRecipe(): mixed { return $this->belongsToMany(BakeryRecipe::class); }

    public function vendor() { return $this->morphTo(); }

    public function scopeTopping(\Illuminate\Database\Eloquent\Builder $query, string $type): void
    {
        $query->where('topping', $type);
    }

    public function scopeUnbaked(\Illuminate\Database\Eloquent\Builder $query): void
    {
        $query->where('baked', false);
    }

    #[Scope]
    protected function fresh(\Illuminate\Database\Eloquent\Builder $query): void
    {
        $query->where('fresh', true);
    }

    public function getLoafNameAttribute(): string { return ''; }

    /** @return Attribute<string> */
    protected function sprinkle(): Attribute
    {
        return new Attribute();
    }
}

class Loaf extends Model
{
    public function getWeight(): int { return 0; }
}

class Baker extends Model
{
    public function getName(): string { return ''; }
}

class BakeryRecipe extends Model
{
    public function getTitle(): string { return ''; }
}

enum JamFlavor: string
{
    case Strawberry = 'strawberry';
    case Raspberry = 'raspberry';
    case Blueberry = 'blueberry';
}

class Frosting
{
    public function __construct(private string $flavor = '') {}
    public function getFlavor(): string { return $this->flavor; }
    public function isSweet(): bool { return $this->flavor !== ''; }
    public function __toString(): string { return $this->flavor; }
}

class FrostingCast
{
    public function get($model, string $key, mixed $value, array $attributes): ?Frosting
    {
        return new Frosting((string) $value);
    }
}
