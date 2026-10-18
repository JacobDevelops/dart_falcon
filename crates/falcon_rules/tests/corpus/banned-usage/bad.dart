import 'support.dart';
import 'support.dart' as p;

void legacy() {}
void example(Client client) {
  legacy(); /* expect: banned-usage */
  var callback = legacy; /* expect: banned-usage */
  client.old(); /* expect: banned-usage */
  var method = client.old; /* expect: banned-usage */
  client..old(); /* expect: banned-usage */
}

void constructorsAndStatics() {
  Client(); /* expect: banned-usage */
  new Client(); /* expect: banned-usage */
  const Client(); /* expect: banned-usage */
  p.Client(); /* expect: banned-usage */
  var factory = Client.new; /* expect: banned-usage */
  Alias.named(); /* expect: banned-usage */
  new Alias.named(); /* expect: banned-usage */
  print(Client.flag); /* expect: banned-usage */
  print(p.Client.flag); /* expect: banned-usage */
  print(Choice.legacy); /* expect: banned-usage */
}
