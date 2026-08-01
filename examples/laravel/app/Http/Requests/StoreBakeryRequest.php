<?php

namespace App\Http\Requests;

use App\Models\BatchSize;
use App\Models\JamFlavor;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;
use Illuminate\Validation\Rules\Enum;

/**
 * The keys of `rules()` are the complete set of inputs this request may
 * carry, so PHPantom offers them as string completion inside
 * `$request->input('…')` and friends — see `Demo::requestInputKeys()`.
 */
class StoreBakeryRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    /**
     * @return array<string, mixed>
     */
    public function rules(): array
    {
        return [
            'name' => 'required|string|max:255',
            'apricot' => 'boolean',
            'dough_temp' => 'nullable|numeric',
            'notes' => 'array',
            'notes.*.body' => 'required|string',
            'owner.email' => 'required|email',
            // An enum rule validates the raw input, so the validated value is
            // the enum's backing type: string here, int for `batch_size`.
            'flavor' => ['required', new Enum(JamFlavor::class)],
            'batch_size' => ['required', Rule::enum(BatchSize::class)],
        ];
    }

    public function withValidator(): void
    {
        // Inside the request itself, `$this` resolves the same rule keys.
        $this->input('name');
    }
}
