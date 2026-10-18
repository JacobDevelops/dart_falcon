class Container {
  Container({Object? alignment, Object? child});
  const Container.named({Object? alignment, Object? child});
}

void unrelated(Object value) {
  Container(alignment: value, child: value);
  new Container(alignment: value, child: value);
  const Container.named(alignment: 1, child: 2);
}
