import 'package:riverpod/riverpod.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart' as flutter;
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
import 'package:state_notifier/state_notifier.dart';
class Counter extends Notifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class AsyncCounter extends AsyncNotifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class StreamCounter extends StreamNotifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class AutoCounter extends AutoDisposeNotifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class FlutterCounter extends flutter.Notifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class HookCounter extends hooks.Notifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class LegacyCounter extends StateNotifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class BaseNotifier extends Notifier<int> {}
class Child extends BaseNotifier {} /* expect: prefer-riverpod-notifier-suffix */
class FamilyCounter extends FamilyNotifier<int, String> {} /* expect: prefer-riverpod-notifier-suffix */
class AsyncFamilyCounter extends FamilyAsyncNotifier<int, String> {} /* expect: prefer-riverpod-notifier-suffix */
class StreamFamilyCounter extends FamilyStreamNotifier<int, String> {} /* expect: prefer-riverpod-notifier-suffix */
class AutoFamilyCounter extends AutoDisposeFamilyNotifier<int, String> {} /* expect: prefer-riverpod-notifier-suffix */
class AutoAsyncFamilyCounter extends AutoDisposeFamilyAsyncNotifier<int, String> {} /* expect: prefer-riverpod-notifier-suffix */
class AutoStreamFamilyCounter extends AutoDisposeFamilyStreamNotifier<int, String> {} /* expect: prefer-riverpod-notifier-suffix */
class BuildlessStreamCounter extends BuildlessStreamNotifier<int> {} /* expect: prefer-riverpod-notifier-suffix */
class AnyCounter extends AnyNotifier<int, int> {} /* expect: prefer-riverpod-notifier-suffix */
