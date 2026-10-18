import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;
Object example() {
  final p0 = BlocProvider<int>(create: (_) => 0, child: BlocProvider<int>.value(value: 1, child: 0)); /* expect: prefer-multi-bloc-provider */
  final p1 = new BlocProvider<int>(create: (_) => 0, child: BlocProvider<int>.value(value: 1, child: 0)); /* expect: prefer-multi-bloc-provider */
  final p2 = fb.BlocProvider<int>(create: (_) => 0, child: BlocProvider<int>.value(value: 1, child: 0)); /* expect: prefer-multi-bloc-provider */
  final p3 = BlocProvider<int>(create: (_) => 0, child: BlocProvider<int>.value(value: 1, child: 0)); /* expect: prefer-multi-bloc-provider */
  final p4 = BlocProvider<int>(create: (_) => 0, child: BlocProvider<int>.value(value: 1, child: 0)); /* expect: prefer-multi-bloc-provider */
  return p0;
}
