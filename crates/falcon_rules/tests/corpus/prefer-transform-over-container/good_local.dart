class Container {
  Container({Object? transform, Object? child});
  const Container.named({Object? transform, Object? child});
}

void unrelated(Object value) {
  Container(transform: value, child: value);
  new Container(transform: value, child: value);
  const Container.named(transform: 1, child: 2);
}
