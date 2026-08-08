<?php

namespace App\Support;

interface CroissantSupplier
{
    public function supply(int $quantity): array;
}
