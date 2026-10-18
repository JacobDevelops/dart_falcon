import 'package:flutter/widgets.dart' as f;

f.Widget leaf() => f.Text('one');
f.Widget atLimit() => f.Center(child: f.Center(child: f.Text('three')));
f.Widget parentheses() => (((((f.Text('one'))))));
f.Widget siblings() => f.Column(children: [f.Text('a'), f.Text('b'), f.Text('c'), f.Text('d'), f.Text('e')]);
f.Widget conditional(bool flag) => f.Center(child: flag ? f.Center(child: f.Text('three')) : f.Text('two'));
f.Widget collectionFor(List<String> names) => f.Column(children: [for (final name in names) f.Text(name)]);
f.Widget extracted(f.Widget subtree) => f.Center(child: subtree);
f.Widget identity(f.Widget widget) => widget;
f.Widget helperBoundary() => f.Center(child: identity(f.Center(child: f.Center(child: f.Text('three')))));

class Deferred extends f.Widget {
  const Deferred(this.builder);
  final f.Widget Function() builder;
}
f.Widget callbackBoundary() => Deferred(() => f.Center(child: f.Center(child: f.Text('three'))));

void shadowed() {
  final Center = (Object value) => value;
  Center(Center(Center(Center('not widget construction'))));
}

class Wrapper {
  Wrapper(Object value);
}
Object unrelatedConstructors() => Wrapper(Wrapper(Wrapper(Wrapper(f.Text('one')))));

f.Widget ignoredSizedBoxes() => f.SizedBox(child: f.SizedBox(child: f.SizedBox(child: f.SizedBox(child: f.Text('one')))));
f.Widget sizedBoxWithinTree() => f.Center(child: f.SizedBox(child: f.Center(child: f.Text('three'))));
