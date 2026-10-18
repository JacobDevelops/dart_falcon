void handle(int x) {
  switch (x) { case 0: break; }
  switch (x) { case 1: break; }
  switch (x) { case 2:
    void local() { switch (x) { case 3: break; } }
    local();
  }
}
