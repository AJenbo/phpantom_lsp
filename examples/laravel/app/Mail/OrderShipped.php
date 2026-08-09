<?php

namespace App\Mail;

use App\Models\BlogPost;
use Illuminate\Mail\Mailable;
use Illuminate\Mail\Mailables\Content;

/**
 * A mailable names its template through the Content its content() returns.
 *
 * Try:
 *  1. Ctrl+Click 'emails.order_shipped' to open the template.
 *  2. Hover $post inside that template — the `with:` argument below is the
 *     call site that types it, the same way a `view()` data array does.
 *  3. Rename a key in `with:` and the template reports it as a variable it
 *     has no use for.
 */
class OrderShipped extends Mailable
{
    public function __construct(private ?BlogPost $post = null)
    {
    }

    public function content(): Content
    {
        return new Content(
            view: 'emails.order_shipped',
            with: ['post' => $this->post],
        );
    }
}
