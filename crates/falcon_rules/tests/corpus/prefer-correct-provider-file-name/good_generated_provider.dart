import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'good_generated_provider.g.dart';

@riverpod
int counter(Ref ref) => 0;

@Riverpod(keepAlive: true)
int cachedCounter(Ref ref) => 0;
