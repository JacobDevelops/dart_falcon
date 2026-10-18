void legacy() {}
class Other {
  void old() {}
}
void example(Other client) {
  void legacy() {}
  legacy();
  client.old();
}
void dynamicReceiver(dynamic client) {
  client.old();
}

class StaticOwner {
  static void legacy() {}
  void run() => legacy();
}
extension ExtensionOwner on int {
  void legacy() {}
  void run() => legacy();
}
enum OtherChoice {
  legacy;
  void run() => print(legacy);
}
class T {}
void generic<T>() => print(T);

extension on String {
  void legacy() {}
  void run() => legacy();
}
class GenericClient<T> {
  const GenericClient.allowed();
}
void genericConstructors() {
  GenericClient<int>.allowed();
  var constructor = GenericClient<int>.allowed;
}
