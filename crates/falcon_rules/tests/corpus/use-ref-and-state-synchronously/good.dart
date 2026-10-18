import 'package:flutter_riverpod/flutter_riverpod.dart';
final provider = Object();
Future<void> pause() async {}
abstract class Count extends Notifier<int> {
  Future<void> beforeAwait() async {
    state += 1;
    await pause();
  }
  Future<void> guarded() async {
    await pause();
    if (ref.mounted) state += 1;
  }
  Future<void> earlyReturn() async {
    await pause();
    if (!ref.mounted) return;
    state += 1;
    ref.read(provider);
  }
  Future<void> explicitGuard() async {
    await pause();
    if (!this.ref.mounted) return;
    this.state = 1;
  }
  Future<void> shortCircuit() async {
    await pause();
    if (ref.mounted && state > 0) state = 2;
  }
  Future<void> shadowed() async {
    int state = 1;
    await pause();
    state += 1;
  }
  Future<void> nested() async {
    await pause();
    void callback() { state = 1; }
    callback();
  }
  Future<void> parenthesizedGuard() async {
    await pause();
    if (!(ref).mounted) return;
    state = 1;
  }
}
class Unrelated {
  int state = 0;
  Future<void> update() async {
    await pause();
    state += 1;
  }
}
abstract class OlderNotifier extends BuildlessNotifier<int> {
  Future<void> update() async {
    await pause();
    state = 1;
  }
}
const alive = true;
abstract class ConstantGuard extends Notifier<int> {
  Future<void> update() async {
    await pause();
    if (ref.mounted == alive) state = 1;
  }
}
