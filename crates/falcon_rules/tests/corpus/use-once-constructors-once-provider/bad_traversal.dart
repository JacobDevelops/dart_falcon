import 'package:riverpod/riverpod.dart';

final providers = [
  OnceProvider((ref) => 1), /* expect: use-once-constructors-once-provider */
];

final nestedCallback = () {
  return FutureProvider((ref) => 2); /* expect: use-once-constructors-once-provider */
};

class ProviderOwner {
  final provider = StateProvider((ref) => 0); /* expect: use-once-constructors-once-provider */
  final Object initialized;

  ProviderOwner()
      : initialized = OnceProvider((ref) => 3); /* expect: use-once-constructors-once-provider */

  Object operator [](int index) =>
      FutureProvider((ref) => index); /* expect: use-once-constructors-once-provider */
}

void scanConditions() {
  if (OnceProvider((ref) => true) != null) { /* expect: use-once-constructors-once-provider */
    return;
  }
  for (final provider in [StateProvider((ref) => 1)]) { /* expect: use-once-constructors-once-provider */
    print(provider);
  }
}

final chained = OnceProvider((ref) => 4).read(); /* expect: use-once-constructors-once-provider */

enum ProviderEnum {
  value;

  Object create() => OnceProvider((ref) => 5); /* expect: use-once-constructors-once-provider */
}

void restoresConstScopes() {
  const constant = [OnceProvider(null)];
  final runtime = [
    const [OnceProvider(null)],
    OnceProvider(null), /* expect: use-once-constructors-once-provider */
  ];
}
