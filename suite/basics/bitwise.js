// test bitwise NOT, AND, XOR, OR

var a = 6, b = 7, ok = 0;

if (~1 === -2) {
    ok++;
}

if ((a & b) === 6) {
    ok++;
}

if ((a ^ b) === 1) {
    ok++;
}

if ((a | b) === 7) {
    ok++;
}

ok === 4;
