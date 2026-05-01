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
        assertType('B', self::retStatic());
        assertType('B', static::retStatic());
        assertType('A', parent::retStatic());
        assertType('B', $this->retStatic());
        assertType('bool', X::retStatic());
        assertType('bool|StaticLateBinding\A', $clUnioned::retStatic());

        assertType('StaticLateBinding\A', A::retStatic(...)());
        assertType('StaticLateBinding\B', B::retStatic(...)());
        assertType('static', self::retStatic(...)());
        assertType('static', static::retStatic(...)());
        assertType('static', parent::retStatic(...)());
        assertType('static', $this->retStatic(...)());
        assertType('bool', X::retStatic(...)());
        assertType('StaticLateBinding\A|bool', $clUnioned::retStatic(...)());

        assertType('A', A::retNonStatic());
        assertType('B', B::retNonStatic());
        assertType('B', self::retNonStatic());
        assertType('B', static::retNonStatic());
        assertType('A', parent::retNonStatic());
        assertType('B', $this->retNonStatic());
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