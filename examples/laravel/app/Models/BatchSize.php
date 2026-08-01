<?php

namespace App\Models;

enum BatchSize: int
{
    case Single = 1;
    case Dozen = 12;
    case Baker = 13;
}
