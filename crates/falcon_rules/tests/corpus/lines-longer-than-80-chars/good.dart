void use(Object o) {}

// See https://github.com/dart-lang/sdk/blob/main/pkg/linter/lib/src/rules/lines_longer.dart
/// Docs: https://dart.dev/tools/linter-rules/lines_longer_than_80_chars#details-and-more
void main() {
  use('https://example.com/a/very/long/path/that/cannot/be/wrapped/at/all/ok/really/long');
  use(r'C:\Users\someone\a\very\long\windows\path\that\is\long\too\x\y\z\w\v\u\t');
  use('''
A multiline string line that is very long and goes past the eighty limit here, way past.
''');
  // ignore: some_lint_name_that_is_long_enough_to_push_this_line_past_eighty_chars_ok
  /*
   * Block comment line with a path segment a/b that is long enough to exceed the limit.
   */
  use('short');
  use('éééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé');
}

int a = 1;
var interpolationComment = '${a /* p/q */} xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx';
