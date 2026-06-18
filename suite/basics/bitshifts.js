// test L, R bit shifts

var a = 5, ok = 0;

if ((a << 1) === 10) {
    ok++;
}

if ((a >> 1) === 2) {
    ok++;
}

return ok === 2;
