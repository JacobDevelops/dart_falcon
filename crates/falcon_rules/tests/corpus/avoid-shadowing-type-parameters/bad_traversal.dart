extension type Box<T>(T value) {
  R convert<T, R>(R Function(T) callback) => callback(value); /* expect: avoid-shadowing-type-parameters */
}

class Holder<T> {
  late T Function<T>(T value) callback; /* expect: avoid-shadowing-type-parameters */
}

class GenericFormal<T> {
  void use(int callback<T>(T value)) {} /* expect: avoid-shadowing-type-parameters */
  void nested(int outer<S>(int inner<S>(S value))) {} /* expect: avoid-shadowing-type-parameters */
}
