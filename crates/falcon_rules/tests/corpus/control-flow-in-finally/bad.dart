int f1() {
  try {
    doThing();
  } finally {
    return 1; /* expect: control-flow-in-finally */
  }
}

void f2() {
  try {
    doThing();
  } finally {
    return; /* expect: control-flow-in-finally */
  }
}

void f3(bool cond) {
  try {
    doThing();
  } finally {
    if (cond) {
      return; /* expect: control-flow-in-finally */
    }
  }
}

void f4() {
  for (var i = 0; i < 10; i++) {
    try {
      doThing();
    } finally {
      break; /* expect: control-flow-in-finally */
    }
  }
}

void f5() {
  for (var i = 0; i < 10; i++) {
    try {
      doThing();
    } finally {
      continue; /* expect: control-flow-in-finally */
    }
  }
}

void f6(bool cond) {
  for (var i = 0; i < 10; i++) {
    try {
      doThing();
    } finally {
      if (cond) break; /* expect: control-flow-in-finally */
    }
  }
}
