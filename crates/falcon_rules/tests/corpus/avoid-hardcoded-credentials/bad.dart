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
