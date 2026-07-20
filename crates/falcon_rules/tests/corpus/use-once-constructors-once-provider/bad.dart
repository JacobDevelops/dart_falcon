import 'package:riverpod/riverpod.dart';

// Bad: OnceProvider without .once() factory

class MyService {
  String getName() => 'service';
}

final serviceProvider = OnceProvider( /* expect: use-once-constructors-once-provider */
  (ref) => MyService(),
);

final anotherProvider = OnceProvider<String>( /* expect: use-once-constructors-once-provider */
  (ref) => 'value',
);

final badPattern = OnceProvider( /* expect: use-once-constructors-once-provider */
  create: (ref) => MyService(),
);

final futureProvider = FutureProvider( /* expect: use-once-constructors-once-provider */
  (ref) async => 'async value',
);

final stateProvider = StateProvider( /* expect: use-once-constructors-once-provider */
  (ref) => 42,
);

// Bad: provider constructed inside constructor arguments
class Holder {
  final Object provider;
  const Holder(this.provider);
}

final nested = new Holder(OnceProvider( /* expect: use-once-constructors-once-provider */
  (ref) => MyService(),
));
