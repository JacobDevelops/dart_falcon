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

// Regular providers (not OnceProvider-related) are fine
final regularProvider = Provider(
  (ref) => 'regular',
);

// OnceProvider variants with .once()
final withTypeParam = OnceProvider<MyService>.once(
  (ref) => MyService(),
);

// Different provider type entirely
final changeNotifierProvider = ChangeNotifierProvider(
  (ref) => MyChangeNotifier(),
);
