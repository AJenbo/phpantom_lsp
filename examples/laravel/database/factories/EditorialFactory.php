<?php

namespace Database\Factories;

use App\Models\BlogAuthor;

/**
 * Nonconventional factory whose model is declared explicitly.
 *
 * There is deliberately no Editorial model: Laravel reads the protected
 * property before trying to derive one from this factory's name.
 */
class EditorialFactory extends BaseFactory
{
    protected $model = BlogAuthor::class;

    public function definition(): array
    {
        return [
            'name' => 'Grace Hopper',
            'email' => 'grace@example.com',
            'genre' => 'technology',
            'active' => true,
        ];
    }
}
