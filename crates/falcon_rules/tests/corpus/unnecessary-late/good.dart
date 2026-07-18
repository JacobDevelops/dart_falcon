// `late` that is genuinely needed, or plain declarations.
late final String deferred;

String loadConfig() => 'cfg';

class Service {
  late final Service instance;
  static final ready = true;
  static int counter = 0;
  late int lazyInstanceField = 1;
  Service();
}

final topLevel = loadConfig();
String plain = 'x';
