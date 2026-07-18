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
