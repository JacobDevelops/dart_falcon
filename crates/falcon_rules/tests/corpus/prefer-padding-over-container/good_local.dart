class Container {
  Container({Object? padding, Object? child});
  const Container.named({Object? padding, Object? child});
}

void unrelated(Object value) {
  Container(padding: value, child: value);
  new Container(padding: value, child: value);
  const Container.named(padding: 1, child: 2);
}
