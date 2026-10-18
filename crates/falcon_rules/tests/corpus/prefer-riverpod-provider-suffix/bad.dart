import 'package:riverpod/riverpod.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart' as flutter;
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final count = Provider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
final future = FutureProvider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
final stream = StreamProvider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
final family = ProviderFamily<int, int>((ref, arg) => arg); /* expect: prefer-riverpod-provider-suffix */
final disposed = AutoDisposeProvider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
final flutterCount = flutter.Provider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
final hooksCount = hooks.Provider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
Provider<int>? typed; /* expect: prefer-riverpod-provider-suffix */
class Providers {
  static final count = Provider<int>((ref) => 0); /* expect: prefer-riverpod-provider-suffix */
}
