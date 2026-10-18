import 'package:flutter/widgets.dart';

class BlocProvider<T> extends StatelessWidget {
  BlocProvider.value({required T value, required Widget child});
}
class RepositoryProvider<T> extends StatelessWidget {
  RepositoryProvider.value({required T value, required Widget child});
}

class BlocBuilder<B, S> extends StatelessWidget {
  BlocBuilder({B? bloc, required Widget Function(BuildContext, S) builder});
}
class BlocListener<B, S> extends StatelessWidget {
  BlocListener({B? bloc, required void Function(BuildContext, S) listener, required Widget child});
}
class BlocConsumer<B, S> extends StatelessWidget {
  BlocConsumer({B? bloc, required Widget Function(BuildContext, S) builder, required void Function(BuildContext, S) listener});
}
class BlocSelector<B, S, T> extends StatelessWidget {
  BlocSelector({B? bloc, required T Function(S) selector, required Widget Function(BuildContext, T) builder});
}
