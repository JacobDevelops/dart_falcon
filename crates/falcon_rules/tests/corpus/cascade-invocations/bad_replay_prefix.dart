import 'bad_replay_prefix.dart' as values;
const first = 1;
const second = 2;
void f() {
  values.first;
  values.second; /* expect: cascade-invocations */
}
