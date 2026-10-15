import 'dart:async';

// leak_detector_visitor.dart matches PropertyAccess assignments by name across the compilation unit.
class TransferOwner {
  late StreamSubscription<int> subscription;

  void transfer(StreamSubscription<int> value) {
    this.subscription = value;
  }
}

class SeparateOwner {
  late StreamSubscription<int> subscription;
}
