{{-- The view of an index component. `<x-card>` resolves to
     App\View\Components\Card\Card — the class inside the directory that
     shares its name — and that class supplies every variable below.

     $bakery      App\Models\Bakery — a public constructor property
     $footer      string — a public property with a default

     Try: hover each one, and trigger completion on `$bakery->`. --}}

<div {{ $attributes->merge(['class' => 'card']) }}>
    <h3>{{ $bakery->flour }}</h3>

    {{ $slot }}

    <footer>{{ $footer }}</footer>
</div>
