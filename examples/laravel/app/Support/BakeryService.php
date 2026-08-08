<?php

namespace App\Support;

class BakeryService
{
    public function bake(string $item): string
    {
        return "a fresh {$item}";
    }
}
