<?php

namespace App\Http\Requests;

/**
 * A request that composes its parent's rules with `array_merge()`.
 *
 * The keys `parent::rules()` contributes are part of this request's contract
 * too, so PHPantom follows the call into `StoreBakeryRequest` and offers the
 * inherited keys alongside the ones declared here — see
 * `Demo::inheritedRequestInputKeys()`.
 */
class UpdateBakeryRequest extends StoreBakeryRequest
{
    /**
     * @return array<string, mixed>
     */
    public function rules(): array
    {
        return array_merge(parent::rules(), [
            'slug' => 'required|string',
            // `exclude` validates the field and then drops it, so it never
            // reaches validated(); `exclude_if` only sometimes does.
            'confirm_slug' => 'required|string|exclude',
            'reason' => 'required|string|exclude_if:slug,legacy',
        ]);
    }
}
