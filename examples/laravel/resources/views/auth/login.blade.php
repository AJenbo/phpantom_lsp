{{-- The counterpart to bakeries/index.blade.php: this template declares no
     signature, so it manages no contract and its call sites are not checked
     at all. That is what keeps call-site validation opt-in — a project adds
     a @bladestan-signature to the templates it wants held to one. --}}

<form method="post" action="{{ route('login') }}">
    @csrf
    <button type="submit">{{ __('messages.welcome') }}</button>
</form>
