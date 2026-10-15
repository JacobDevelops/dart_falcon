import 'dart:async';
import 'holder.dart' as holder;

void prefixedWrite(Stream<int> stream) {
  final topSubscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  holder.topSubscription = topSubscription;
}

class LocalHolder {
  StreamSubscription<int>? subscription; /* expect: cancel-subscriptions */
  void transfer(holder.ForeignHolder foreign) {
    foreign.subscription = subscription;
  }
}
