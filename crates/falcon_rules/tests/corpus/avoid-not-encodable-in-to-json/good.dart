class Serializable {
  Object toJson() => {'name': 'value'};
}
class Inherited extends Serializable {}
class Example {
  Object toJson(dynamic unknown, Object broad) => {
    'string': 'value',
    'number': 1,
    'double': 1.5,
    'boolean': true,
    'null': null,
    'list': [1, 'value', false, null],
    'map': {'nested': [true]},
    'object': Serializable(),
    'inherited': Inherited(),
    'unknown': unknown,
    'broad': broad,
  };
}
class OtherMethods {
  Object unrelated() => <int>{1};
  Object toJson() {
    Object helper() => <int>{1};
    final callback = () => <int>{2};
    return {'ok': true};
  }
}
class Direct {
  Object toJson() => 'value';
  Object customToJson() => [1, 2];
}
class Spread {
  Object toJson(Set<String> values) => [...values];
}
class Optional {
  Object toJson([int value = 1]) => value;
}
class OptionalHolder {
  Object toJson() => {'value': Optional()};
}

class MissingParent extends MissingBase {}
class MissingContract implements MissingInterface {}
class IndirectMissing extends MissingParent {}
class IncompleteModels {
  Object toJson() => {
    'parent': MissingParent(),
    'contract': MissingContract(),
    'indirect': IndirectMissing(),
  };
}
