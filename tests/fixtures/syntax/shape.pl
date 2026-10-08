package Outer::Shape;
use strict;
use List::Util qw(max);
use parent -norequire, 'Base::Thing';
require Data::Dumper;
no warnings;
use constant LIMIT => 10;

sub new {
    my ($class) = @_;
    return bless {}, $class;
}

sub area : lvalue {
    my $self = shift;
}

package Outer::Other {
    sub run { 1 }
    sub helper {
        my $local = 1;
    }
}

package Outer::Shape::Circle;

sub area { 2 }
1;
