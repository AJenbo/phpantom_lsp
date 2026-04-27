<?php // lint >= 8.2

namespace StandaloneTypes;

use function PHPStan\Testing\assertType;

function foo(): null {
	return null;
}
function bar(): false {
	return false;
}

class standalone {
	public false $f = false;
	public null $n = null;

	function foo(): null {
		return null;
	}
	function bar(): false {
		return false;
	}
}

function takesNull(null $n) {
	assertType('null', $n);
}

function takesFalse(false $f) {
	assertType('false', $f);
}


function doFoo() {
	assertType('null', foo());
	assertType('false', bar());

	$s = new standalone();

	assertType('null', $s->foo());
	assertType('false', $s->bar());

	assertType('null', $s->n);
	assertType('false', $s->f);
}
