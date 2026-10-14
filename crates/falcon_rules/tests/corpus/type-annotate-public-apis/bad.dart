var answer = 42; /* expect: type-annotate-public-apis */
final nothing = null; /* expect: type-annotate-public-apis */
late final pending; /* expect: type-annotate-public-apis */

install(String id) {} /* expect: type-annotate-public-apis */

void register(id, String url) {} /* expect: type-annotate-public-apis */

typedef Handler(event); /* expect: type-annotate-public-apis */

class Widget {
  var count = 0; /* expect: type-annotate-public-apis */
  static var shared; /* expect: type-annotate-public-apis */
  Widget(key); /* expect: type-annotate-public-apis */
  build() => null; /* expect: type-annotate-public-apis */
  void update(value) {} /* expect: type-annotate-public-apis */
  get label => ''; /* expect: type-annotate-public-apis */
  set label(v) {} /* expect: type-annotate-public-apis */
  operator +(Widget other) => this; /* expect: type-annotate-public-apis */
}
