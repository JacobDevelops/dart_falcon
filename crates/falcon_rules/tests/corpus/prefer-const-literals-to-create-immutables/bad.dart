import 'package:flutter/widgets.dart';
import 'package:meta/meta.dart';
import 'package:meta/meta.dart' as meta;

@immutable
class Config {
  const Config(this.values, {this.names = const {}, this.lookup = const {}});

  final Object values;
  final Object names;
  final Object lookup;
}

class Special extends Config {
  const Special(super.values);
}

class Tags extends StatelessWidget {
  const Tags({super.key, required this.tags});

  final List<String> tags;

  @override
  Widget build(BuildContext context) => const SizedBox();
}

const int base = 3;

void literals() {
  print(Config([1, 2, base])); /* expect: prefer-const-literals-to-create-immutables */
  print(Config(const [], names: {'a', 'b'})); /* expect: prefer-const-literals-to-create-immutables */
  print(Config(const [], lookup: {'a': 1})); /* expect: prefer-const-literals-to-create-immutables */
  print(Config(([1]))); /* expect: prefer-const-literals-to-create-immutables */
  print(Config(<int>[])); /* expect: prefer-const-literals-to-create-immutables */
  print(Config([[1], const [2]])); /* expect: prefer-const-literals-to-create-immutables */ /* expect: prefer-const-literals-to-create-immutables */
  print(Tags(tags: ['x', 'y'])); /* expect: prefer-const-literals-to-create-immutables */
  print(Special([1])); /* expect: prefer-const-literals-to-create-immutables */
}

@immutable
abstract mixin class Base {
  const Base();
}

class Derived extends Base {
  const Derived(this.data);
  final Object data;
}

void inherited() {
  const local = 1;
  print(Derived({'a': local})); /* expect: prefer-const-literals-to-create-immutables */
  print(Derived(<String>{'x'})); /* expect: prefer-const-literals-to-create-immutables */
}

// Constructor-form meta annotations have the same immutable identity.
@Immutable('')
class ReasonedImmutable {
  const ReasonedImmutable(this.values);
  final List<Object> values;
}

void constructorAnnotation() {
  print(ReasonedImmutable([])); /* expect: prefer-const-literals-to-create-immutables */
}

@Immutable('Collection arguments must remain immutable')
class ReasonedBase {
  const ReasonedBase(this.values);
  final Object values;
}

class ReasonedChild extends ReasonedBase {
  const ReasonedChild(super.values);
}

void inheritedConstructorAnnotation() {
  print(ReasonedChild([1])); /* expect: prefer-const-literals-to-create-immutables */
}

@meta.Immutable('')
class PrefixedImmutable {
  const PrefixedImmutable(this.values);
  final Object values;
}

void prefixedConstructorAnnotation() {
  print(PrefixedImmutable({'key': 1})); /* expect: prefer-const-literals-to-create-immutables */
}
