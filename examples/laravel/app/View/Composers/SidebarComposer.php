<?php

namespace App\View\Composers;

use App\Models\BlogAuthor;
use Illuminate\View\View;

/**
 * A view composer: Laravel runs `compose()` for every view its registration
 * targets, and whatever it passes to `$view->with()` is in that view's scope.
 *
 * The registration lives in DemoServiceProvider::boot(); the template that
 * reads these variables is resources/views/partials/sidebar.blade.php.
 */
class SidebarComposer
{
    public function compose(View $view): void
    {
        $view->with('sidebarAuthor', BlogAuthor::query()->firstOrFail())
            ->with('sidebarPostCount', BlogAuthor::query()->count());
    }
}
