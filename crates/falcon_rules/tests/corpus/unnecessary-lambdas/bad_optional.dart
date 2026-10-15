const one = 1;
void named({int value = 1}) {}
void optional(int value, [int another = 1]) {}
void nullable([int? value]) {}
void requiredNamed({required int value}) {}
void pair({int first = 1, int second = 2}) {}
class OptionalMaker { OptionalMaker([int value = 1]); }
class NamedOptionalMaker {
  NamedOptionalMaker.named([int? value]);
  NamedOptionalMaker.required(int? value);
}
class GenericOptionalMaker<T> { GenericOptionalMaker.named([T? value]); }
const required = Required();
class Required { const Required(); }
void annotatedNamed({@required int? value}) {}
void annotatedDefault({@required int value = 1}) {}
class OptionalError extends ArgumentError { OptionalError([super.message]); }
class OptionalBase { OptionalBase([int? value]); }
class OptionalChild extends OptionalBase { OptionalChild([super.value]); }

void forwarding() {
  final a = ({int value = 1}) => named(value: value); /* expect: unnecessary-lambdas */
  final b = (int value, [int another = 1]) => optional(value, another); /* expect: unnecessary-lambdas */
  final c = ({int value = one}) => named(value: value); /* expect: unnecessary-lambdas */
  final d = ({int value = 1 + 0}) => named(value: value); /* expect: unnecessary-lambdas */
  final e = ([int? value = null]) => nullable(value); /* expect: unnecessary-lambdas */
  final g = ({required int value}) => requiredNamed(value: value); /* expect: unnecessary-lambdas */
  // The analyzer does not compare default values before suggesting a tear-off.
  final h = ({int value = 2}) => named(value: value); /* expect: unnecessary-lambdas */
  final i = (int value, [int another = 2]) => optional(value, another); /* expect: unnecessary-lambdas */
  final j = ([int? value = 1]) => nullable(value); /* expect: unnecessary-lambdas */
  final k = ({int first = 1, int second = 2}) => pair(second: second, first: first); /* expect: unnecessary-lambdas */
  final l = ({int first = 1}) => pair(first: first); /* expect: unnecessary-lambdas */
  final m = ([int value = 1]) => OptionalMaker(value); /* expect: unnecessary-lambdas */
  final n = ([int value = 1]) => new OptionalMaker(value); /* expect: unnecessary-lambdas */
  final namedConstructor = ([int? value]) => new NamedOptionalMaker.named(value); /* expect: unnecessary-lambdas */
  final requiredConstructor = (int? value) => new NamedOptionalMaker.required(value); /* expect: unnecessary-lambdas */
  final genericConstructor = ([int? value]) => new GenericOptionalMaker<int>.named(value); /* expect: unnecessary-lambdas */
  final annotated = ({int? value}) => annotatedNamed(value: value); /* expect: unnecessary-lambdas */
  final annotatedWithDefault = ({int value = 1}) => annotatedDefault(value: value); /* expect: unnecessary-lambdas */
  final optionalError = ([Object? message]) => OptionalError(message); /* expect: unnecessary-lambdas */
  final explicitOptionalError = ([Object? message]) => new OptionalError(message); /* expect: unnecessary-lambdas */
  final optionalSuper = ([int? value]) => OptionalChild(value); /* expect: unnecessary-lambdas */
  final explicitOptionalSuper = ([int? value]) => new OptionalChild(value); /* expect: unnecessary-lambdas */
  void local([int value = one]) {}
  final o = ([int value = 1]) => local(value); /* expect: unnecessary-lambdas */
  void Function({int first, int second}) block = ({int first = 1, int second = 2}) { /* expect: unnecessary-lambdas */
    pair(first: first, second: second);
  };
}
