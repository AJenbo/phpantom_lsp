<?php
use Illuminate\Support\Facades\Route;

Route::prefix('v1')->group(function () {
    Route::get('/users', fn() => [])->name('api.v1.users.index');
});
