<?php
namespace App\Models;

use Foo\Bar;
use Foo\Baz as Qux;
use Foo\{One, Two as Deux};
use function Foo\helper;
require_once __DIR__ . '/boot.php';
include 'lib/util.php';

const LIMIT = 10;

#[Entity]
class Account
{
    const RATE = 2;
    private $balance;

    public function make(int $a) { return $a; }

    public static function create() {
        $local = 1;
    }
}

interface Shape
{
    public function area(): float;
}

trait Greets
{
    public function hello() {}
}

enum Suit: string
{
    case Hearts = 'H';
    case Spades = 'S';
}

function make() {}

namespace App\Other;

function other() {}
