{{-- Demonstrates variable completion inside included partials --}}
@php
/**
 * @bladestan-signature
 * @var ?\App\Models\BlogPost $post
 */
@endphp

<div style="font-family: sans-serif; padding: 20px;">
    <h2>{{ __('messages.welcome') }}</h2>

    @if($post)
        <p>{{ $post->getTitle() }} has been published!</p>
        <p>Published: {{ $post->created_at->diffForHumans() }}</p>
    @endif
</div>
