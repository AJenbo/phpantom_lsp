<?php

/**
 * PHPantomLSP Demo File
 *
 * This file demonstrates the hover functionality of PHPantomLSP.
 * Try hovering over the word "PHPantom" below to see the special message!
 */

namespace Demo;

class PHPantomDemo
{
    /**
     * Welcome to PHPantom - the phantom PHP Language Server!
     *
     * Hover over "PHPantom" anywhere in this file to see the LSP in action.
     */
    public function demonstrateHover(): void
    {
        // PHPantom provides basic LSP functionality
        $message = "PHPantom is a minimal LSP server written in Rust";

        echo $message . PHP_EOL;
    }

    /**
     * Regular PHP code - hovering over these words won't trigger special behavior
     */
    public function regularCode(): string
    {
        $variable = "This is regular code";
        $function = "Some function";

        // Only "PHPantom" triggers the hover message
        return "PHPantom rocks!";
    }
}

// Create an instance and run the demo
$demo = new PHPantomDemo();
$demo->demonstrateHover();

// More PHPantom references for testing
// Try hovering over PHPantom in comments too!
$phantom = "PHPantom";

?>
