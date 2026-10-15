class Values { bool contains(Object? value) => true; }
void joined(bool condition, Values values) {
  var predicate = condition ? values.contains : (String value) => value.isEmpty; /* expect: avoid-types-on-closure-parameters */
}

class Data {}
class Call {}
typedef MessageHandler = Future<Data?>? Function(Data? message);
class Messenger {
  void setMessageHandler(String name, MessageHandler? handler) {}
}
class Channel {
  final Messenger binaryMessenger = Messenger();
  final String name = '';
  void setHandler(Future<dynamic> Function(Call)? handler) {
    binaryMessenger.setMessageHandler(
      name,
      handler == null ? null : (Data? message) => handle(message, handler), /* expect: avoid-types-on-closure-parameters */
    );
  }
  Future<Data?> handle(Data? message, Future<dynamic> Function(Call) handler) async => null;
}

typedef Callback = int Function(int);
Callback accept(Callback callback) => callback;
void collections(bool condition, List<int> values) {
  List<Callback> list = [
    (int value) => value,
    if (condition) (int value) => value,
    for (final value in values) (int argument) => argument,
    accept((int value) => value), /* expect: avoid-types-on-closure-parameters */
    condition
        ? (int value) => value /* expect: avoid-types-on-closure-parameters */
        : (int value) => value, /* expect: avoid-types-on-closure-parameters */
  ];
  Set<Callback> set = {
    (int value) => value,
    if (condition) (int value) => value,
    for (final value in values) (int argument) => argument,
    accept((int value) => value), /* expect: avoid-types-on-closure-parameters */
  };
  Map<Callback, Callback> map = {
    (int key) => key: (int value) => value,
    if (condition) (int key) => key: (int value) => value,
    for (final value in values) (int key) => key: (int argument) => argument,
    accept((int key) => key): /* expect: avoid-types-on-closure-parameters */
        accept((int value) => value), /* expect: avoid-types-on-closure-parameters */
  };
  List<Callback> spread = [...[(int value) => value]];
  List<List<Callback>> nested = [[(int value) => value]];
}
