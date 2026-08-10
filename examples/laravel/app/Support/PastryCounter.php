<?php

namespace App\Support;

class PastryCounter
{
    public function counted(string $item): int
    {
        return strlen($item);
    }

    public function tally(): static
    {
        return $this;
    }
}
