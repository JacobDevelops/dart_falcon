import 'dart:async';
import 'dart:collection';

Iterable<int> getValues() => [];

void bad(
  Iterable<int> values,
  Map<String, int> map,
  List<int> list,
  Set<int> set,
  Queue<int> queue,
  String text,
  Future<int> future,
) {
  values.any(null); /* expect: null-closures */
  getValues().any(null); /* expect: null-closures */
  values.firstWhere(null, orElse: null); /* expect: null-closures */ /* expect: null-closures */
  values.fold(0, null); /* expect: null-closures */
  map.putIfAbsent('x', null); /* expect: null-closures */
  list.removeWhere(null); /* expect: null-closures */
  set.retainWhere(null); /* expect: null-closures */
  queue.removeWhere(null); /* expect: null-closures */
  text.replaceAllMapped('x', null); /* expect: null-closures */
  text.splitMapJoin('x', onMatch: null); /* expect: null-closures */
  future.then(null, onError: null); /* expect: null-closures */ /* expect: null-closures */
  Future.microtask(null); /* expect: null-closures */
  Future.doWhile(null); /* expect: null-closures */
  Timer.run(null); /* expect: null-closures */
  List.generate(2, null); /* expect: null-closures */
  scheduleMicrotask(null); /* expect: null-closures */
}

extension UnrelatedMap on int {
  Object? map(Object? callback) => null;
}

void unrelatedExtensionDoesNotSuppress(List<int> values) {
  values.map(null); /* expect: null-closures */
}

class ConstructorBase {
  ConstructorBase(Object? value);
}

class ConstructorInitializers extends ConstructorBase {
  ConstructorInitializers(List<int> values)
      : field = values.map(null), /* expect: null-closures */
        assert(values.any(null)), /* expect: null-closures */
        super(values.where(null)); /* expect: null-closures */

  ConstructorInitializers.redirect(List<int> values)
      : this(values.map(null)); /* expect: null-closures */

  final Iterable<int> field;
}
