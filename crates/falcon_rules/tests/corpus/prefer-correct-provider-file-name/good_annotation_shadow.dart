import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'good_annotation_shadow.g.dart';

class Riverpod {
  const Riverpod({bool keepAlive = false});
}
const riverpod = Riverpod();

@riverpod
int counter(Object ref) => 0;

@Riverpod(keepAlive: true)
int cachedCounter(Object ref) => 0;
