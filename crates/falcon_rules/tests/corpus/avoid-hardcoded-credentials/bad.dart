const password = 'not-a-real-password'; /* expect: avoid-hardcoded-credentials */
class Settings {
  final _apiKey = 'fixture-key'; /* expect: avoid-hardcoded-credentials */
  String clientSecret = '';
  void update() {
    clientSecret = 'fixture-secret'; /* expect: avoid-hardcoded-credentials */
    this.clientSecret = 'other-fixture-secret'; /* expect: avoid-hardcoded-credentials */
  }
}
const ACCESS_TOKEN = 'fixture-token'; /* expect: avoid-hardcoded-credentials */

void connect({String password = 'fixture-password'}) {} /* expect: avoid-hardcoded-credentials */
void authenticate([String apiKey = 'fixture-key']) {} /* expect: avoid-hardcoded-credentials */
void configure({String client_secret = ('fixture-secret')}) {} /* expect: avoid-hardcoded-credentials */
void unlock({int passwd = 1234}) {} /* expect: avoid-hardcoded-credentials */
void authorize([double authToken = 1.5]) {} /* expect: avoid-hardcoded-credentials */

class Connection {
  final String password;
  Connection({this.password = 'fixture-password'}); /* expect: avoid-hardcoded-credentials */
  Connection.positional([this.password = 'fixture-password']); /* expect: avoid-hardcoded-credentials */
  Connection.named({String secret = 'fixture-secret'}) : password = secret; /* expect: avoid-hardcoded-credentials */
  void login({String accessToken = 'fixture-token'}) {} /* expect: avoid-hardcoded-credentials */
  void refresh([String privateKey = 'fixture-key']) {} /* expect: avoid-hardcoded-credentials */
}

class ChildConnection extends Connection {
  ChildConnection({super.password = 'fixture-password'}); /* expect: avoid-hardcoded-credentials */
  ChildConnection.positional([super.password = 'fixture-password']) : super.positional(); /* expect: avoid-hardcoded-credentials */
}

void callbacks() {
  void local([String _PASSWORD = 'fixture-password']) {} /* expect: avoid-hardcoded-credentials */
  final named = ({String api_key = 'fixture-key'}) {}; /* expect: avoid-hardcoded-credentials */
  final positional = ([String clientSecret = 'fixture-secret']) {}; /* expect: avoid-hardcoded-credentials */
}
