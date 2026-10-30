// Deliberately unavailable package members must not be treated as one identity.
import 'package:unavailable/state.dart';

class Consumer extends ExternalState {
  Future<void> update(Future<int> amount) async {
    first[0] = second[0] + await amount;
    first[0] = first[0] + await amount;
    first.count = second.count + await amount;
    first.count = first.count + await amount;
  }
}
