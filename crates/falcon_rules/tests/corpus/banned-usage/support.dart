class Client {
  const Client();
  const Client.named();
  void old() {}
  static int flag = 0;
}
typedef Alias = Client;
enum Choice { legacy, modern }
