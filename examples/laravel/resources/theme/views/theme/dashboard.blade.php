<x-app-layout>
    {{-- Lives under the custom view path registered in config/view.php --}}
    {{-- No @var signature here: $theme and $author are inferred from the
         view('theme.dashboard', …) call site in app/Demo.php.
         Try: hover $theme (string), type $author-> for BlogAuthor members. --}}
    <h1 class="{{ $theme }}">Theme Dashboard</h1>
    <p>Curated by {{ $author->name }}</p>
</x-app-layout>
