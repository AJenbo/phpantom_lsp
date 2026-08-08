{{-- The view of a class-backed component: every variable below comes
     from App\View\Components\PostSummary, which Laravel resolves from
     this view's name (components.post-summary → PostSummary). No caller
     passes any of them.

     $post        App\Models\BlogPost — a public constructor property
     $heading     string — a public property with a default
     $wordCount   the argument-less wordCount() method, wrapped in
                  Illuminate\View\InvokableComponentVariable so it both
                  prints and calls

     Try: hover each one, and trigger completion on `$post->`. --}}

<article {{ $attributes->merge(['class' => 'post-summary']) }}>
    <h3>{{ $heading }}</h3>

    {{-- $post is a BlogPost, so its relations and accessors resolve --}}
    <p>{{ $post->getTitle() }} by {{ $post->author->name }}</p>

    {{-- Printed as a value, and called as a function: both are legal --}}
    <small>{{ $wordCount }} words ({{ $wordCount() }})</small>

    {{-- excerpt() takes an argument, so it is not a variable here.
         Uncomment to see the undefined-variable diagnostic:
         {{ $excerpt }} --}}

    {{ $slot }}
</article>
