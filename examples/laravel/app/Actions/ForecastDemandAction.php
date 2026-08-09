<?php

namespace App\Actions;

use Illuminate\Console\Attributes\Aliases;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;

/**
 * An Artisan command that lives nowhere the convention would look: its class
 * name does not end in `Command` and its directory is not `Console/Commands`.
 * `bootstrap/app.php` registers it with `withCommands([... app/Actions])`,
 * which is all Laravel needs.
 *
 * PHPantom indexes it anyway, so `bakery:forecast` completes, navigates, and
 * validates like any other command, and so does the `bakery:fc` alias the
 * `#[Aliases]` attribute declares.
 */
#[Signature('bakery:forecast {region} {--window=14 : Days of demand to project}')]
#[Aliases(['bakery:fc'])]
class ForecastDemandAction extends Command
{
    protected $description = 'Project bakery demand for a region';

    public function handle(): int
    {
        $region = $this->argument('region');
        $window = $this->option('window');

        $this->info("Forecasting {$region} over {$window} days");

        return self::SUCCESS;
    }
}
