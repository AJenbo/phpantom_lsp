<?php

namespace StaticLateBinding;

use function PHPStan\Testing\assertType;

class A
{
    public static function retStaticConst(): int
    {
        return 1;
    }

    /**
     * @return static
     */
    public static function retStatic()
    {
        return new static();
    }

    /**
     * @return static
     */
    public function retNonStatic()
    {
        return new static();
    }

    /**
     * @param-out int $out
     */
    public static function outStaticConst(&$out): int
    {
        $out = 1;
    }
}

class B extends A
{
    /**
     * @return int
     */
    public static function retStaticConst(): int
    {
        return 2;
    }

    /**
     * @param-out int $out
     */
    public static function outStaticConst(&$out): int
    {
        $out = 2;
    }

    public function foo(): void
    {
        $clUnioned = mt_rand() === 0
            ? A::class
            : X::class;

        assertType('int', A::retStaticConst());
        assertType('int', B::retStaticConst());
        assertType('int', self::retStaticConst());
        assertType('int', static::retStaticConst());
        assertType('int', parent::retStaticConst());
        assertType('int', $this->retStaticConst());
        assertType('bool', X::retStaticConst());
        assertType('*ERROR*', $clUnioned->retStaticConst());

        assertType('int', A::retStaticConst(...)());
        assertType('int', B::retStaticConst(...)());
        assertType('int', self::retStaticConst(...)());
        assertType('int', static::retStaticConst(...)());
        assertType('int', parent::retStaticConst(...)());
        assertType('int', $this->retStaticConst(...)());
        assertType('bool', X::retStaticConst(...)());
        assertType('mixed', $clUnioned->retStaticConst(...)());

        assertType('StaticLateBinding\A', A::retStatic());
        assertType('StaticLateBinding\B', B::retStatic());
        assertType('static(StaticLateBinding\B)', self::retStatic());
        assertType('static(StaticLateBinding\B)', static::retStatic());
        assertType('static(StaticLateBinding\B)', parent::retStatic());
        assertType('static(StaticLateBinding\B)', $this->retStatic());
        assertType('bool', X::retStatic());
        assertType('bool|StaticLateBinding\A', $clUnioned::retStatic());

        assertType('StaticLateBinding\A', A::retStatic(...)());
        assertType('StaticLateBinding\B', B::retStatic(...)());
        assertType('static(StaticLateBinding\B)', self::retStatic(...)());
        assertType('static(StaticLateBinding\B)', static::retStatic(...)());
        assertType('static(StaticLateBinding\B)', parent::retStatic(...)());
        assertType('static(StaticLateBinding\B)', $this->retStatic(...)());
        assertType('bool', X::retStatic(...)());
        // Upstream records `mixed` here and notes it should be the union
        // (phpstan/phpstan#11687); we resolve the union, so keep it.
        assertType('bool|StaticLateBinding\A', $clUnioned::retStatic(...)());

        assertType('static(StaticLateBinding\B)', A::retNonStatic());
        assertType('static(StaticLateBinding\B)', B::retNonStatic());
        assertType('static(StaticLateBinding\B)', self::retNonStatic());
        assertType('static(StaticLateBinding\B)', static::retNonStatic());
        assertType('static(StaticLateBinding\B)', parent::retNonStatic());
        assertType('static(StaticLateBinding\B)', $this->retNonStatic());
        assertType('bool', X::retNonStatic());
        assertType('*ERROR*', $clUnioned->retNonStatic());
    }
}

class X
{
    public static function retStaticConst(): bool
    {
        return false;
    }

    /**
     * @param-out bool $out
     */
    public static function outStaticConst(&$out): void
    {
        $out = false;
    }

    public static function retStatic(): bool
    {
        return false;
    }

    public function retNonStatic(): bool
    {
        return false;
    }
}