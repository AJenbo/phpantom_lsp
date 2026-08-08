{{-- A template's signature is a contract its callers are checked against.
     BakeryController::index() passes exactly what is declared here; leave
     $bakeries out of that call, or pass a key this template never reads,
     and the call site is flagged.
     Try: hover $bakeries, or type $bakery-> inside the loop. --}}
@php
/**
 * @bladestan-signature
 * @var \Illuminate\Database\Eloquent\Collection<int, \App\Models\Bakery> $bakeries
 */
@endphp

<ul>
    @foreach($bakeries as $bakery)
        <li>{{ $bakery->croissant }} at {{ $bakery->dough_temp }}&deg;</li>
    @endforeach
</ul>
