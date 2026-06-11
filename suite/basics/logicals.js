// test logical NOT, OR, AND

var a = 67, b, c = false, ok = 0;

if ((a || b) === 67) {
    ok++;
}

if ((b && a) === undefined) {
    ok++;
}

if (!c === true) {
    ok++;
}

ok === 3;
