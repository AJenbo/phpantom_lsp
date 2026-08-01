<?php

namespace App\View\Components;

use App\Models\BlogPost;
use Illuminate\View\Component;
use Illuminate\View\View;

/**
 * A Blade component, written the way every Laravel project writes one.
 *
 * `view('name')` is declared to return the `Illuminate\Contracts\View\View`
 * contract, but the view factory always builds the concrete
 * `Illuminate\View\View`, which is what `render(): View` names here.  The
 * resolver follows the concrete class, so the signature checks out and
 * completion on the result offers the concrete view's members.
 *
 * Try:
 *  1. Hover `view(...)` below to see `Illuminate\View\View`.
 *  2. Trigger completion on `view('welcome')->` to see the concrete
 *     view's members.
 *  3. Ctrl+Click "welcome" to open resources/views/welcome.blade.php.
 */
class WelcomeCard extends Component
{
    public function render(): View
    {
        return view('welcome', ['posts' => BlogPost::all()]);
    }
}
