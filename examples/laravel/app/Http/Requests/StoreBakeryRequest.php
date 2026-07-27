<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

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
        ];
    }

    public function withValidator(): void
    {
        // Inside the request itself, `$this` resolves the same rule keys.
        $this->input('name');
    }
}
