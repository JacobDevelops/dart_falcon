import 'package:meta/meta.dart';

@immutable
class Config {
  const Config(this.values, {this.names = const {}});

  final Object values;
  final Object names;
}

class Mutable {
  Mutable(this.values);

  final Object values;
}

class Lookalike implements Config {
  Lookalike(this.values);

  @override
  final Object values;

  @override
  Object get names => const {};
}

void literals(int x, List<int> other) {
  print(Config(const [1, 2]));
  print(const Config([1, 2], names: {'a'}));
  print(Config([x]));
  print(Config([...other]));
  print(Config([for (var i = 0; i < 3; i++) i]));
  print(Config([if (x > 0) 1]));
  print(Mutable([1, 2]));
  final values = [1, 2];
  print(Config(values));
  print(Config(other.toList(), names: {x}));
  print(Unknown([1]));
  print(Lookalike([1]));
}

class EqualityKey {
  const EqualityKey();
  @override
  bool operator ==(Object other) => other is EqualityKey;
  @override
  int get hashCode => 0;
}

void keysWithoutPrimitiveEquality() {
  print(Config({EqualityKey(): 1}));
  print(Config({1.5}));
}

@Immutable('')
class ReasonedImmutable {
  const ReasonedImmutable(this.values);
  final List<Object> values;
}

void constructorAnnotation(List<Object> values) {
  print(ReasonedImmutable(const []));
  print(const ReasonedImmutable([]));
  print(ReasonedImmutable(values));
}

class ReasonedInterface implements ReasonedImmutable {
  const ReasonedInterface(this.values);
  @override
  final List<Object> values;
}

void constructorAnnotationInterface() {
  print(ReasonedInterface([]));
}
