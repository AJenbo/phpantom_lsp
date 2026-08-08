<?php

namespace App\Support;

class PlainOven
{
    public function bake(string $item): string
    {
        return "a plain {$item}";
    }
}
