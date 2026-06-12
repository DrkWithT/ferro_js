// test == operator

var a, b = null, c = true, d = 1, ok = 0;

if (1 == 1) {
    ok++;
}

if (a == b) {
    ok++;
}

if (c == d) {
    ok++;
}

if (this != NaN) {
    ok++;
}

ok === 4;
