<?php

namespace App\Mail;

use App\Models\BlogAuthor;
use App\Models\BlogPost;
use Illuminate\Mail\Mailable;

/**
 * The older mailable shape: build() names the template through
 * $this->view(), which is a render only because the receiver is a mailable.
 *
 * emails/blog_published.blade.php declares $post and $author, yet nothing
 * here passes them: a mailable hands its view every public property it
 * declares, so neither is reported missing.
 */
class BlogPublished extends Mailable
{
    public function __construct(
        public BlogPost $post,
        public BlogAuthor $author,
    ) {
    }

    public function build(): static
    {
        return $this->view('emails.blog_published');
    }
}
