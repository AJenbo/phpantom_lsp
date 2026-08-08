<?php

namespace App\View\Components\Card;

use App\Models\Bakery;
use Illuminate\View\Component;
use Illuminate\View\View;

/**
 * An index component: the class repeats the name of the directory it sits
 * in, so Blade lets a tag address it by the directory alone.
 * `<x-card>` reaches this class, not `App\View\Components\Card`, which is
 * a namespace rather than a class.
 *
 * No transform of the view name predicts that, so the component is only
 * findable by having seen the file. PHPantom indexes the component
 * namespaces up front, which is what makes the view below resolve.
 *
 * Try:
 *  1. Open resources/views/components/card.blade.php and hover $bakery.
 *  2. Trigger completion on `$bakery->` there.
 */
class Card extends Component
{
    public function __construct(
        public Bakery $bakery,
        public string $footer = '',
    ) {
    }

    public function render(): View
    {
        return view('components.card');
    }
}
