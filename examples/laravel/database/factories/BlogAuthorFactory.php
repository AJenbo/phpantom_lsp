<?php

namespace Database\Factories;

/**
 * Convention-based factory.
 *
 * There is no generic annotation here on purpose: PHPantom derives the model
 * (App\Models\BlogAuthor) from the concrete factory class name even though it
 * inherits Laravel's Factory through BaseFactory. It also synthesizes the
 * dynamic has{Relationship}() / for{Relationship}() methods (hasPosts(),
 * hasProfile(), forPosts(), forProfile()) for each relationship on the model —
 * each returning the factory so the chain continues.
 */
class BlogAuthorFactory extends BaseFactory
{
    public function definition(): array
    {
        return [
            'name' => 'Ada Lovelace',
            'email' => 'ada@example.com',
            'genre' => 'science',
            'active' => true,
        ];
    }
}
