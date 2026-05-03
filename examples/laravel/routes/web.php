<?php
use Illuminate\Support\Facades\Route;

Route::get('/', fn() => view('welcome'))->name('home');

Route::prefix('admin')->group(function () {
    Route::get('/users', fn() => view('admin.users.index'))->name('admin.users.index');
});
