import 'dart:async';

StreamSubscription<int>? topSubscription;
void dispose() => topSubscription?.cancel();

class ForeignHolder {
  StreamSubscription<int>? subscription;
  void dispose() => subscription?.cancel();
}
