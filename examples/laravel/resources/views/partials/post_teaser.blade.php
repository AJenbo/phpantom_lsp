{{-- Rendered only from another template, and declaring nothing at all: no
     signature, no @var, no @props. The type of $teaser comes from the
     @include in welcome.blade.php that passes it, the same way a
     controller's view() call types the page it renders.

     A partial that does declare a signature is the better contract (see
     post_row.blade.php next door), because then the callers are checked
     against it rather than the other way round. This one is what an
     unannotated project gets for free. --}}

<p class="teaser">
    {{-- Try: $teaser-> --}}
    {{ $teaser->getTitle() }} by {{ $teaser->author->name }}
</p>
