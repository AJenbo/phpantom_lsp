{{-- Demonstrates completion and navigation in nested Blade views --}}
@php
/**
 * @bladestan-signature
 * @var \App\Models\AuthorCollection $users
 */
@endphp

@extends('welcome')

@section('content')
    <h1>{{ __('messages.welcome') }} - Admin</h1>

    <table>
        <thead>
            <tr>
                <th>Name</th>
                <th>Email</th>
                <th>Role</th>
            </tr>
        </thead>
        <tbody>
            @foreach($users->active()->byName() as $user)
                <tr>
                    <td>{{ $user->name }}</td>
                    <td>{{ $user->email }}</td>
                </tr>
            @endforeach
        </tbody>
    </table>

    @if($users->isEmpty())
        <p>{{ trans('pagination.next') }}</p>
    @endif
@endsection
