class Equality<T> {
  const Equality();
  bool equals(List<T> a, List<T> b) => true;
}

// A receiver has no downward context; absence must not become Unknown context.
bool compare(List<int> a, List<int> b) =>
    Equality().equals(a, b); /* expect: prefer-const-constructors */

class Box<T> {
  const Box(this.value);
  final T value;
}

// Real contextual unknown type parameters must still prevent a const rewrite.
Box<T?> contextual<T>() => Box(null);

void argument<T>(void Function(Box<T?>) sink) {
  sink(Box(null));
}

abstract class AsyncValue<T> {
  const AsyncValue();
  const factory AsyncValue.loading() = AsyncLoading<T>;
}

class AsyncLoading<T> extends AsyncValue<T> {
  const AsyncLoading();
}

// The analyzer approximates an assignment's context using its RHS result type.
class Element<T> {
  var value = AsyncValue<T>.loading();

  void clear() {
    value = AsyncLoading();
  }
}

void assignmentShapes<T>(AsyncValue<T>? value) {
  // A ??= expression joins its original target and RHS types.
  value ??= AsyncLoading(); /* expect: prefer-const-constructors */
  value = (AsyncLoading());
  value = AsyncLoading()..toString();
}

// A supertype return context cannot be viewed as the constructed subclass.
AsyncValue<T> loading<T>() =>
    AsyncLoading(); /* expect: prefer-const-constructors */
AsyncValue<T> blockLoading<T>() {
  return AsyncLoading(); /* expect: prefer-const-constructors */
}
AsyncValue<int> concreteLoading() =>
    AsyncLoading(); /* expect: prefer-const-constructors */

abstract class TypeConverter<D, S> {
  const TypeConverter();
}

mixin JsonTypeConverter<D, S> on TypeConverter<D, S> {}

class ExtensionConverter<E, I extends Object> extends TypeConverter<E, I>
    with JsonTypeConverter<E, I> {
  const ExtensionConverter();
}

JsonTypeConverter<E, I> converter<E, I extends Object>() {
  return ExtensionConverter(); /* expect: prefer-const-constructors */
}

JsonTypeConverter<E, I> converterArrow<E, I extends Object>() =>
    ExtensionConverter(); /* expect: prefer-const-constructors */

class NullableValue<T> {
  const NullableValue.ofNullable(T? value)
      : assert(value != null || null is! T);
}

void constructorTypeTests() {
  // Making these const would evaluate a failing constructor assertion.
  () => NullableValue<int?>.ofNullable(null);
  () => NullableValue<String?>.ofNullable(null);
  // Non-nullable type arguments and non-null values satisfy the assertion.
  NullableValue<int>.ofNullable(null); /* expect: prefer-const-constructors */
  NullableValue<int?>.ofNullable(1); /* expect: prefer-const-constructors */
}
