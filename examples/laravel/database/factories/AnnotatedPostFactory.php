<?php

namespace Database\Factories;

use App\Models\BlogPost;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * Factory annotated the way `make:factory` generates one, with an explicit
 * `@extends Factory<Model>` binding.
 *
 * The generic binding resolves create()/make() on its own, but PHPantom
 * still synthesizes forAuthor()/hasAuthor() and trashed() from BlogPost's
 * relationships and SoftDeletes trait, since Factory::__call() resolves
 * those independently of the generics system.
 *
 * @extends Factory<BlogPost>
 */
class AnnotatedPostFactory extends Factory
{
    protected $model = BlogPost::class;

    public function definition(): array
    {
        return [
            'title' => 'On Annotated Factories',
            'slug' => 'on-annotated-factories',
            'published' => true,
        ];
    }
}
