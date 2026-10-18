import 'package:flutter_bloc/flutter_bloc.dart' as fb;

class Snapshot { int count = 0; }
class Counter extends fb.Cubit<Snapshot> {
  Counter() : super(Snapshot());
  void unchanged() {
    emit(state); /* expect: emit-new-bloc-state-instances */
    this.emit(this.state..count++); /* expect: emit-new-bloc-state-instances */
  }
}
