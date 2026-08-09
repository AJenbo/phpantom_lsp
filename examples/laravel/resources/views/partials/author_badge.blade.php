{{-- Rendered by the @include in welcome.blade.php, which passes no data at
     all: Blade hands an included partial the scope the directive is written
     in, so $user arrives from the including template.

     What it holds under that name is checked against the signature below,
     not just the fact that it holds one. welcome.blade.php declares $user as
     a ?BlogAuthor, which is what this partial asks for; a template that held
     something else there would be reported where it includes this one. --}}
@php
/**
 * @bladestan-signature
 * @var ?\App\Models\BlogAuthor $user
 */
@endphp

<span class="author-badge">
    {{-- Try: $user?-> --}}
    {{ $user?->displayName ?? __('messages.welcome') }}
</span>
