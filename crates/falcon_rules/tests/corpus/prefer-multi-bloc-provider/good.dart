import 'package:flutter_bloc/flutter_bloc.dart' as fb;
class BlocProvider { BlocProvider({required Object child}); }
Object example() {
  final single = fb.BlocProvider<int>(create: (_) => 0, child: 0);
  final unrelated = BlocProvider(child: BlocProvider(child: 0));
  final mixed = fb.BlocProvider<int>(create: (_) => 0, child: BlocProvider(child: 0));
  final repositories = fb.RepositoryProvider<int>(create: (_) => 0, child: fb.RepositoryProvider<int>(create: (_) => 0, child: 0));
  final multi = fb.MultiBlocProvider(providers: [fb.BlocProvider<int>(create: (_) => 0, child: 0)], child: single);
  return multi;
}
