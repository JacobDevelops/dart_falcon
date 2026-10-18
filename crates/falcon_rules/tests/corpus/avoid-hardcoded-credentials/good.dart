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
