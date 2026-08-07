<?php
use App\Http\Controllers\BakeryController;
use Illuminate\Support\Facades\Route;

Route::get('/', fn() => view('welcome'))->name('home');
Route::get('/bakeries', [BakeryController::class, 'index'])->name('bakeries.index');

// A macro registered in RouteServiceProvider.  The routes its body declares
// (login, logout, password.request, password.update) belong to this file, and
// the group prefixes in force at the call site reach them.
Route::bakeryAuth();

Route::prefix('admin')->group(function () {
    Route::get('/users', fn() => view('admin.users.index'))->name('admin.users.index');
});

Route::prefix('bakeries')
    ->controller(BakeryController::class)
    ->group(function () {
        Route::get('{bakery}', 'show')->name('bakeries.show');
        Route::patch('{bakery}/cancel', 'cancel')->name('bakeries.cancel');
    });

// A resource registration names no URI: Laravel derives one from the resource
// name, singularizing each segment to build the {parameters}.  This nested
// name yields bakeries/{bakery}/ovens and bakeries/{bakery}/ovens/{oven}.
Route::resource('bakeries.ovens', BakeryController::class)->only(['index', 'show']);
