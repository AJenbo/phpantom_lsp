<?php

namespace App\Console\Commands;

use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;

/**
 * Artisan command whose signature comes from the `#[Signature]` attribute
 * (Laravel 13).
 *
 * PHPantom parses the attribute exactly like a `$signature` property, so the
 * command name (`bakery:prune-stale`) completes / navigates / validates, and
 * the `--days` option drives own-parameter and array-key completion.
 */
#[Signature('bakery:prune-stale {--days=7 : Discard loaves older than this many days}')]
class PruneStaleLoavesCommand extends Command
{
    protected $description = 'Discard stale loaves';

    public function handle(): int
    {
        $days = $this->option('days');

        $this->info("Pruning loaves older than {$days} days");

        return self::SUCCESS;
    }
}
