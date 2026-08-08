{{-- An anonymous component that shows the whole variable-declaration
     chain. Each source only supplies what the ones above it left out:

     1. the @bladestan-signature docblock — the template's contract, and
        the only place a real type can be written. It wins outright.
     2. @props — one entry per attribute the component consumes. An entry
        with a default is typed from that default; a bare entry is a
        *required* prop, so its type is whatever the caller passes.
     3. Blade's own component scope: $attributes, $slot, $componentName.
     4. types inferred from view() call sites, for templates that declare
        nothing at all.

     Try: hover each variable below and check what type it resolved to. --}}
@php
    /**
     * @bladestan-signature
     * @var \App\Models\BlogAuthor $author
     */
@endphp
@props([
    'author',
    'heading',
    'variant' => 'info',
    'collapsed' => false,
    'tags' => [],
])

{{-- $author: BlogAuthor. The signature names it, so the bare @props entry
     does not water it down to an untyped required prop. Try: $author-> --}}
<div {{ $attributes->merge(['class' => "panel panel-{$variant}"]) }}
    data-component="{{ $componentName }}">

    {{-- $heading: a required prop the signature does not type, so it stays
         unknown rather than being invented as null — passing it to a
         string parameter is not an error. --}}
    <h3>{{ strtoupper($heading) }}</h3>

    {{-- $variant: string, and $collapsed: bool, both from their defaults --}}
    @unless ($collapsed)
        <p>By {{ $author->name }} ({{ $author->email }})</p>
    @endunless

    {{-- $tags: array, so a foreach over it is understood --}}
    @foreach ($tags as $tag)
        <span class="tag">{{ $tag }}</span>@if (!$loop->last), @endif
    @endforeach

    {{ $slot }}
</div>
