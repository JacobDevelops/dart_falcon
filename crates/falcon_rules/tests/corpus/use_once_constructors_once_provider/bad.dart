import 'package:riverpod/riverpod.dart';

// Bad: OnceProvider without .once() factory

class MyService {
  String getName() => 'service';
}

final serviceProvider = OnceProvider( /* expect: use_once_constructors_once_provider */
  (ref) => MyService(),
);

final anotherProvider = OnceProvider<String>( /* expect: use_once_constructors_once_provider */
  (ref) => 'value',
);

final badPattern = OnceProvider( /* expect: use_once_constructors_once_provider */
  create: (ref) => MyService(),
);

final futureProvider = FutureProvider( /* expect: use_once_constructors_once_provider */
  (ref) async => 'async value',
);

final stateProvider = StateProvider( /* expect: use_once_constructors_once_provider */
  (ref) => 42,
);
