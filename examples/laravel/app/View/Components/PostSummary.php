<?php

namespace App\View\Components;

use App\Models\BlogPost;
use Illuminate\View\View;
use Illuminate\View\Component;

/**
 * A class-backed Blade component: the class supplies the variables its
 * view reads.
 *
 * Blade merges a component class's public properties and its public
 * argument-less methods into the data the view renders with, so
 * resources/views/components/post-summary.blade.php can read $post,
 * $heading, and $wordCount without a caller ever passing them.
 *
 * Try:
 *  1. Open resources/views/components/post-summary.blade.php and hover
 *     each of those variables.
 *  2. Add a public property here and watch it appear in the view.
 */
class PostSummary extends Component
{
    public function __construct(
        public BlogPost $post,
        public string $heading = 'Summary',
    ) {
    }

    /**
     * An argument-less public method, which the view reads as a variable:
     * `{{ $wordCount }}` prints it and `{{ $wordCount() }}` calls it.
     */
    public function wordCount(): int
    {
        return str_word_count($this->post->getTitle());
    }

    /**
     * A method that takes an argument is not view data: Blade hands it to
     * the view as a bare closure, so the view calls it on the component
     * rather than reading a variable.
     */
    public function excerpt(int $length): string
    {
        return substr($this->post->getTitle(), 0, $length);
    }

    public function render(): View
    {
        return view('components.post-summary');
    }
}
