String readSecret() => '';
final password = readSecret();
const apiKey = '';
const secretCount = 3;
const token = 'not-a-credential-name';
const accessToken = 'access_token';
const _authToken = 'AUTH-TOKEN';
class StorageKeys {
  static const password = 'password';
}
void update(String supplied) {
  final clientSecret = supplied;
  final accessToken = 'prefix-$supplied';
}

const configuredSecret = 'fixture-configured-value';
void connect({
  String password = '',
  String? passwd = null,
  String clientSecret = configuredSecret,
  String apiKey = 'API-KEY',
  String token = 'not-a-credential-name',
  int secretCount = 3,
}) {}
void authenticate([String password = 'password']) {}
void storageKey([String _access_token = 'accessToken']) {}
void requiredCredentials(String password, {required String apiKey}) {}

class Connection {
  final String password;
  Connection({this.password = 'password'});
  Connection.positional([this.password = configuredSecret]);
  void login({String authToken = 'auth_token'}) {}
  void refresh([String privateKey = configuredSecret]) {}
}

class ChildConnection extends Connection {
  ChildConnection({super.password = ''});
  ChildConnection.positional([super.password = 'password']) : super.positional();
}

void callbacks() {
  void local({String secret = 'secret'}) {}
  final named = ({String apiKey = configuredSecret}) {};
  final positional = ([String clientSecret = 'CLIENT-SECRET']) {};
}
