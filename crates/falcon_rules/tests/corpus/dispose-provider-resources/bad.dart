import 'package:riverpod/riverpod.dart';

class Channel { Channel(); Channel.named(); void close() {} }
class Subscription { Subscription(); void cancel() {} }
class Service { Service(); void dispose() {} }
class Child extends Channel {}
final channel = Provider((ref) {
  final resource = Channel(); /* expect: dispose-provider-resources */
  return resource;
});
final subscription = Provider((ref) {
  final resource = Subscription(); /* expect: dispose-provider-resources */
  return resource;
});
final named = Provider((ref) {
  final resource = Channel.named(); /* expect: dispose-provider-resources */
  return resource;
});
final explicitNew = Provider((ref) {
  final resource = new Channel(); /* expect: dispose-provider-resources */
  return resource;
});
final inherited = Provider((ref) {
  final resource = Child(); /* expect: dispose-provider-resources */
  return resource;
});
final emptyCleanup = Provider((ref) {
  final resource = Channel(); /* expect: dispose-provider-resources */
  ref.onDispose(() {});
  return resource;
});
final wrongCleanup = Provider((ref) {
  final first = Channel(); /* expect: dispose-provider-resources */
  final second = Channel();
  ref.onDispose(second.close);
  return first;
});
