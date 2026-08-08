{{-- Demonstrates the variables a service provider puts in scope.

     Nothing here declares $siteName, $supplier, $sidebarAuthor, or
     $sidebarPostCount, and no `view('partials.sidebar')` call passes them.
     They come from app/Providers/DemoServiceProvider.php: the first two from
     `View::share()`, which reaches every template, and the last two from the
     `View::composer('partials.*', SidebarComposer::class)` registration,
     which reaches only the views under `partials.`. --}}

<aside>
    {{-- Shared by View::share(): available in every template --}}
    <h3>{{ $siteName }}</h3>

    {{-- A shared object keeps its class, so its members resolve:
         Try: $supplier-> --}}
    <p>{{ count($supplier->supply(12)) }} croissants on order</p>

    {{-- From SidebarComposer::compose(): $sidebarAuthor is a
         \App\Models\BlogAuthor, so its members and casts resolve.
         Try: $sidebarAuthor-> --}}
    <p>Editor: {{ $sidebarAuthor->name }} ({{ $sidebarAuthor->displayName }})</p>
    <p>{{ $sidebarPostCount }} authors</p>
</aside>
