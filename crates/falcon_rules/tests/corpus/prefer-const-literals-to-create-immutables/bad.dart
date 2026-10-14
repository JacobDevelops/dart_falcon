import 'package:flutter/widgets.dart';
import 'package:meta/meta.dart';

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
