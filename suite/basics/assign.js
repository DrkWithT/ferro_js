// test assignment
var a = 1, b, ok = 0;

a = a + 1;

if (a === 2) {
    ok++;
}

b = a = a + 1;

if (b === 3) {
    ok++;
}

return ok === 2;
