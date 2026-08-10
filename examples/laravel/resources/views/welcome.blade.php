{{-- Try: Ctrl+Click on variables, config keys, route names, and translation keys --}}
@php
/**
 * @bladestan-signature
 * @var ?\App\Models\BlogAuthor $user
 * @var \App\Models\PostCollection $posts
 */
@endphp

<!DOCTYPE html>
<html lang="{{ config('app.locale') }}">
<head>
    <title>{{ config('app.name') }} - {{ __('messages.welcome') }}</title>
</head>
<body>
    <h1>{{ __('messages.welcome') }}</h1>

    {{-- Comment contents are inert: an apostrophe ('), a commented-out
         {{ $echo }}, and a mention of @endphp all stay inside the comment --}}

    {{-- Variable completion: $user-> triggers member suggestions --}}
    @if($user)
        <p>Hello, {{ $user->name }}!</p>
        <p>Email: {{ $user->email }}</p>
    @endif

    {{-- The inline @php(…) directive assigns just like the block form:
         $featured is a \App\Models\BlogPost from here on --}}
    @php($featured = $posts->published()->byNewest()->first())
    @if($featured)
        <p>Featured: {{ $featured->getTitle() }}</p>
    @endif

    {{-- @error injects an implicit $message variable --}}
    @error('email')
        <div class="alert">{{ $message }}</div>
    @enderror

    {{-- @session injects an implicit $value variable --}}
    @session('status')
        <div class="success">{{ $value }}</div>
    @endsession

    {{-- @forelse with @empty — supported since recently --}}
    @forelse($posts->published()->byNewest() as $post)
        {{-- Standalone @var docblock for type narrowing in foreach --}}
        @php /** @var \App\Models\BlogPost $post */ @endphp
        <article>
            <h2>{{ $post->getTitle() }}</h2>
            <p>By {{ $post->author->name }}</p>
            <span>{{ $post->created_at->diffForHumans() }}</span>

            {{-- $loop variable is injected automatically in foreach/forelse --}}
            @if($loop->first)
                <span class="badge">Featured</span>
            @endif
            @if(!$loop->last)
                <hr>
            @endif
        </article>
    @empty
        <p>No posts found.</p>
    @endforelse

    {{-- Route and translation helpers (Ctrl+Click navigates to definition) --}}
    <nav>
        {{-- Go-to-definition works on view references in @include, @extends --}}
        <a href="{{ route('home') }}">{{ __('messages.welcome') }}</a>
        <a href="{{ route('admin.users.index') }}">{{ trans('auth.failed') }}</a>
    </nav>

    {{-- @include: Ctrl+Click navigates to the included view --}}
    @include('emails.order_shipped', ['post' => $posts->first()])

    {{-- partials/sidebar.blade.php reads variables nothing passes it: a
         provider shares them, or a view composer writes them. --}}
    @include('partials.sidebar')

    {{-- An @include hands the partial the scope it is written in, so
         partials/author_badge.blade.php gets $user without this directive
         passing anything. The type comes with it: what this template holds
         under $user is checked against the partial's signature. --}}
    @include('partials.author_badge')

    {{-- @includeFirst renders the first candidate that exists, so a project
         can override a partial without the caller changing. Both names are
         navigable, and a candidate that exists nowhere is the point of the
         directive rather than a typo, so it is not reported. --}}
    @includeFirst(['partials.sidebar_override', 'partials.sidebar'])

    {{-- @each renders its partial once per entry, with the entry bound to
         the name the third argument spells and $key to its key. The types
         come from the collection: $posts is a PostCollection, so $post is a
         BlogPost inside partials/post_row.blade.php. --}}
    <table>
        @each('partials.post_row', $posts, 'post')
    </table>

    {{-- The <x-alert> component view is where $attributes and $slot come
         from: resources/views/components/alert.blade.php --}}
    <x-alert class="mt-4">{{ __('messages.welcome') }}</x-alert>

    {{-- A bound attribute whose expression is wrapped over several lines
         (what a formatter does to a long array) is still read as one PHP
         expression. Try: Ctrl+Click __ and hover $posts on the last line.

         alert.blade.php declares no @props at all, yet reads $messages
         from this very attribute — every attribute a tag passes becomes
         a variable inside the component, whether @props names it or
         not. See components/alert.blade.php. --}}
    <x-alert class="mt-4"
        :messages="[
            __('messages.welcome'),
            trans('auth.failed'),
            $posts->first()?->title,
        ]" />

    {{-- The <x-panel> component view shows how a signature docblock,
         @props defaults, and Blade's own component scope layer into one
         set of variables: resources/views/components/panel.blade.php --}}
    <x-panel :author="$posts->first()?->author" heading="Latest" variant="warning">
        {{ __('messages.welcome') }}
    </x-panel>

    {{-- <x-post-summary> is backed by a class, App\View\Components\PostSummary.
         Its view reads the class's public members, not what this tag
         passes: resources/views/components/post-summary.blade.php --}}
    <x-post-summary :post="$posts->first()" heading="Latest post" />

    {{-- A component tag is the call Laravel makes with it, so its
         attributes are the constructor's arguments and are checked as
         such: change `:post` to pass a BlogAuthor and the tag is reported
         the way any other call would be. Attributes the component
         forwards to its attribute bag (`class` below) name no parameter,
         so they are not arguments and are never reported.

         The tag also binds $component to the component itself, the same
         variable Blade's own compiled output uses, so the body reaches
         the class rather than only the data it exposes.
         Try: completion on `$component->` on the next line. --}}
    <x-post-summary :post="$posts->first()" heading="Second post" class="mt-2">
        {{ $component->excerpt(20) }}
    </x-post-summary>

    {{-- <x-widgets::badge> is not a namespaced view: the `widgets::` prefix
         is registered in DemoServiceProvider::boot() with
         Blade::anonymousComponentNamespace(), so the tag renders the plain
         view components/widgets/badge.blade.php. The attributes below are
         where that template's $label and $author come from. --}}
    <x-widgets::badge label="Author" :author="$posts->first()?->author" />

    {{-- <x-card> names a directory, not a class: Blade falls back to the
         class inside it that repeats its name, App\View\Components\Card\Card.
         Its view reads that class's members:
         resources/views/components/card.blade.php --}}
    @php($bakery = \App\Models\Bakery::firstOrFail())
    <x-card :bakery="$bakery" footer="Baked today" />

    {{-- @verbatim: content inside is skipped by the preprocessor --}}
    @verbatim
        <p>This {{ $blade }} syntax is not processed</p>
    @endverbatim

    {{-- Conditional rendering with config --}}
    @if(config('app.debug'))
        <pre>Debug mode is on (env: {{ config('app.env') }})</pre>
    @endif

    {{-- A section's two halves live in different files: this renders what
         a child fills. Ctrl+Click the name to jump to every child that
         fills it, and typing a name inside @yield('') offers the ones the
         children already write. --}}
    @yield('content')

    {{-- The same for stacks: children @push here. --}}
    @stack('scripts')
</body>
</html>
