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

    {{-- Variable completion: $user-> triggers member suggestions --}}
    @if($user)
        <p>Hello, {{ $user->name }}!</p>
        <p>Email: {{ $user->email }}</p>
    @endif

    {{-- Foreach with model completion — $posts->byNewest() uses PostCollection --}}
    @foreach($posts->byNewest() as $post)
        <article>
            <h2>{{ $post->title }}</h2>
            <p>By {{ $post->author->name }}</p>
            <span>{{ $post->created_at->diffForHumans() }}</span>
        </article>
    @endforeach

    {{-- Route and translation helpers --}}
    <nav>
        <a href="{{ route('home') }}">{{ __('messages.welcome') }}</a>
        <a href="{{ route('admin.users.index') }}">{{ trans('auth.failed') }}</a>
    </nav>

    {{-- Includes and nested views --}}
    @include('emails.order_shipped', ['post' => $posts->first()])

    {{-- Conditional rendering with config --}}
    @if(config('app.debug'))
        <pre>Debug mode is on (env: {{ config('app.env') }})</pre>
    @endif
</body>
</html>
