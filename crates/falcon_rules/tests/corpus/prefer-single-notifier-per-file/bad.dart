import 'package:riverpod/riverpod.dart' as r;
abstract class First extends r.Notifier<int> {}
abstract class Second extends r.AsyncNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class Third extends r.StreamNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class Fourth extends r.AutoDisposeNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class _Private extends r.StateNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class Indirect extends First {} /* expect: prefer-single-notifier-per-file */
abstract class Family extends r.FamilyNotifier<int, String> {} /* expect: prefer-single-notifier-per-file */
abstract class AsyncFamily extends r.FamilyAsyncNotifier<int, String> {} /* expect: prefer-single-notifier-per-file */
abstract class StreamFamily extends r.FamilyStreamNotifier<int, String> {} /* expect: prefer-single-notifier-per-file */
abstract class AutoFamily extends r.AutoDisposeFamilyNotifier<int, String> {} /* expect: prefer-single-notifier-per-file */
abstract class AutoAsyncFamily extends r.AutoDisposeFamilyAsyncNotifier<int, String> {} /* expect: prefer-single-notifier-per-file */
abstract class AutoStreamFamily extends r.AutoDisposeFamilyStreamNotifier<int, String> {} /* expect: prefer-single-notifier-per-file */
abstract class Buildless extends r.BuildlessNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class BuildlessAsync extends r.BuildlessAsyncNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class BuildlessStream extends r.BuildlessStreamNotifier<int> {} /* expect: prefer-single-notifier-per-file */
abstract class Any extends r.AnyNotifier<int, int> {} /* expect: prefer-single-notifier-per-file */
