// test NaN and Infinity

var n = NaN, i = Infinity, ok = 0;

if (n !== n) {
    ok++;
}

if (1 / 0 === i) {
    ok++;
}

if (0 / 0 !== NaN) {
    ok++;
}

return ok === 3;
