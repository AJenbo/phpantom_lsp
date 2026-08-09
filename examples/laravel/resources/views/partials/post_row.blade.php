{{-- Rendered by the @each in welcome.blade.php, once per entry of the
     collection it names. Only two variables reach here: the entry, under
     the name @each's third argument spells, and $key. Everything the
     calling template holds stays behind, unlike @include.

     The item's type is the element type of the collection that was handed
     over, and $key's is its key type, so the signature below is checked
     against what welcome.blade.php actually iterates. --}}
@php
/**
 * @bladestan-signature
 * @var \App\Models\BlogPost $post
 * @var array-key $key
 */
@endphp

<tr>
    <td>{{ $key }}</td>

    {{-- Try: $post-> --}}
    <td>{{ $post->getTitle() }}</td>
    <td>{{ $post->author->name }}</td>
</tr>
