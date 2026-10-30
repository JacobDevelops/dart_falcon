class Cubit<T> { Cubit(T value); }
class Local extends Cubit<int> {
  Local() : super(0);
  int count() => 1;
  String label() => 'local';
  Future<int> load() async => 1;
  value() => 2;
  static int utility() => 3;
}
