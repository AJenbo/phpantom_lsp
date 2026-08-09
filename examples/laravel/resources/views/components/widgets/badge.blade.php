{{-- An anonymous component reached through a registered prefix. Its view
     name is the ordinary `components.widgets.badge`, but callers write
     `<x-widgets::badge>`, because DemoServiceProvider::boot() runs
     `Blade::anonymousComponentNamespace('components/widgets', 'widgets')`.

     Nothing in this file mentions the prefix, so the registration is what
     pairs the tag with this template. Without it the attributes those tags
     pass would reach nothing, and $author below would be undefined.

     Try: hover $author and $label, and trigger completion on `$author->`. --}}

<span {{ $attributes->merge(['class' => 'badge']) }}>
    {{-- $label comes from the plain `label="…"` attribute the tag writes,
         and $author from the bound `:author="…"` one, resolved to whatever
         the caller's expression is. Neither is named in a @props. --}}
    <strong>{{ $label }}</strong>
    {{ $author->name }} &lt;{{ $author->email }}&gt;
</span>
