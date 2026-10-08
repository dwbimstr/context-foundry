import 'package:flutter/material.dart';
import 'src/util.dart' as util;
import 'dart:async' show Future;
part 'store.g.dart';

const limit = 10;
final greeting = 'hi';
var counter = 0;

@immutable
class Store {
  final int field = 1;

  Store();
  Store.named();

  @override
  void put(int a) {}

  int get size => 0;

  static Store create() => Store();
}

abstract class Shape {
  double area();
}

mixin Greets {
  void hello() {}
}

extension StoreX on Store {
  void extra() {}
}

enum Color { red, green }

typedef Name = String;

int helper() {
  var local = 1;
  return local;
}
