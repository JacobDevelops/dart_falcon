// Bad: createState contains logic beyond returning a new State instance.
import 'package:flutter/material.dart';

class W1 extends StatefulWidget {
  @override
  State<W1> createState() { /* expect: no-logic-in-create-state */
    print('creating');
    return _W1State();
  }
}

class W2 extends StatefulWidget {
  @override
  State<W2> createState() => _W2State()..init(); /* expect: no-logic-in-create-state */
}

class W3 extends StatefulWidget {
  @override
  State<W3> createState() { /* expect: no-logic-in-create-state */
    final state = _W3State();
    return state;
  }
}

class W4 extends StatefulWidget {
  @override
  State<W4> createState() => _W4State(this); /* expect: no-logic-in-create-state */
}

class W5 extends StatefulWidget {
  @override
  State<W5> createState() { /* expect: no-logic-in-create-state */
    return _W5State()..counter = 0;
  }
}

class _W1State extends State<W1> {}
class _W2State extends State<W2> { void init() {} }
class _W3State extends State<W3> {}
class _W4State extends State<W4> { _W4State(Object value); }
class _W5State extends State<W5> { int counter = 0; }
