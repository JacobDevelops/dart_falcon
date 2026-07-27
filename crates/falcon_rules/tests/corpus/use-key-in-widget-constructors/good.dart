import 'package:flutter/widgets.dart';

class SuperKey extends StatelessWidget {
  const SuperKey({super.key});
}

class ForwardedKey extends StatelessWidget {
  const ForwardedKey({Key? key}) : super(key: key);
}

class CoalescedKey extends StatelessWidget {
  const CoalescedKey({Key? key}) : super(key: key ?? const Key('fallback'));
}

class HardCodedKey extends StatelessWidget {
  const HardCodedKey() : super(key: const Key('fixed'));
}

class RedirectedKey extends StatelessWidget {
  const RedirectedKey({Key? key}) : this.named(key: key ?? const Key('fallback'));
  const RedirectedKey.named({Key? key}) : super(key: key);
}

class _PrivateWidget extends StatelessWidget {}

class PrivateConstructor extends StatelessWidget {
  const PrivateConstructor._();
}

class FactoryWidget extends StatelessWidget {
  factory FactoryWidget() => throw UnimplementedError();
}
