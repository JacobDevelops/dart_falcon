import 'dart:async';
import 'package:riverpod/riverpod.dart' as rp;
class Channel { Channel(); void close() {} }
final prefixed = rp.Provider((ref) => Channel()); /* expect: dispose-provider-resources */
final future = rp.FutureProvider((ref) async {
  final channel = Channel(); /* expect: dispose-provider-resources */
  return channel;
});
final stream = rp.StreamProvider((ref) {
  final channel = Channel(); /* expect: dispose-provider-resources */
  return Stream.value(channel);
});
final family = rp.Provider.family<Channel, int>((ref, id) {
  final channel = Channel(); /* expect: dispose-provider-resources */
  return channel;
});
