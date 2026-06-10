import 'package:riverpod/riverpod.dart';

// Good: OnceProvider with .once() factory

class MyService {
  String getName() => 'service';
}

final serviceProvider = OnceProvider.once(
  (ref) => MyService(),
);

final anotherProvider = OnceProvider<String>.once(
  (ref) => 'value',
);

final goodPattern = OnceProvider.once(
  create: (ref) => MyService(),
);

final futureProvider = FutureProvider.once(
  (ref) async => 'async value',
);

final stateProvider = StateProvider.once(
  (ref) => 0,
);
