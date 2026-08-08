<?php

namespace App\Http\Controllers;

use App\Models\Bakery;
use Illuminate\Http\JsonResponse;
use Illuminate\View\View;

class BakeryController
{
    public function index(): View
    {
        // bakeries/index.blade.php declares `$bakeries` and nothing else, so
        // this is the whole contract this call has to satisfy.
        return view('bakeries.index', [
            'bakeries' => Bakery::where('open', true)->freshlyBaked()->get(),
        ]);
    }

    public function show(Bakery $bakery): JsonResponse
    {
        return response()->json([
            'id' => $bakery->id,
            'name' => $bakery->loaf_name,
        ]);
    }

    public function cancel(Bakery $bakery): JsonResponse
    {
        return response()->json([
            'cancelled' => $bakery->id,
        ]);
    }
}
