<?php

namespace App;

use App\Models\BlogAuthor;
use App\Models\BlogPost;
use App\Models\PostCollection;
use Illuminate\Contracts\Support\Arrayable;

/**
 * The data welcome.blade.php is rendered from, as an object rather than
 * an array.
 *
 * The view factory converts an Arrayable with toArray() before rendering,
 * so `view('welcome', new WelcomeData())` hands the template whatever the
 * shape below promises, and that shape is what the template's signature is
 * checked against.
 *
 * @implements Arrayable<string, mixed>
 */
class WelcomeData implements Arrayable
{
    /**
     * @return array{user: ?BlogAuthor, posts: PostCollection}
     */
    public function toArray(): array
    {
        return [
            'user' => BlogAuthor::first(),
            'posts' => BlogPost::where('published', true)->get(),
        ];
    }
}
