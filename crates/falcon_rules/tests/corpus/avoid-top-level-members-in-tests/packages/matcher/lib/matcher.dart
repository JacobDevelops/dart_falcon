abstract class Matcher {}
class _Matcher implements Matcher { const _Matcher(); }
const Matcher isTrue = _Matcher();
const Matcher isFalse = _Matcher();
const Matcher isNull = _Matcher();
const Matcher isNotNull = _Matcher();
const Matcher isEmpty = _Matcher();
const Matcher completes = _Matcher();
const Matcher emitsDone = _Matcher();
const Matcher throwsStateError = _Matcher();
Matcher equals(dynamic value) => const _Matcher();
Matcher completion(dynamic value) => const _Matcher();
Matcher throwsA(dynamic value) => const _Matcher();
Matcher emits(dynamic value) => const _Matcher();
Matcher emitsInOrder(dynamic value) => const _Matcher();
Matcher anyOf(dynamic value) => const _Matcher();
Matcher predicate(dynamic callback) => const _Matcher();
Matcher isA<T>() => const _Matcher();
Matcher isNot(dynamic value) => const _Matcher();
