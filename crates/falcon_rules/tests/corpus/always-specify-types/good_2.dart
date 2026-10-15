import 'package:meta/meta.dart';

// `@optionalTypeArgs` types, static members, and `_` parameters.
@optionalTypeArgs
mixin Loose<T> {}

class Seq<T> {
  static const Seq<Never> empty = Seq<Never>._();
  const Seq._();
}

Seq<Never> none(Loose loose) => Seq.empty;

class Sink {
  set first(_) {}
}
