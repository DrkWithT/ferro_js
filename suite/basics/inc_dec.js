var x = 0, a = ++x, b = x++, c = 4, d = --c, e = c--, ok = 0;

if (a === 1) {
    ok++;
}

if (b === 1) {
    ok++;
}

if (x === 2) {
    ok++;
}

if (d === 3) {
    ok++;
}

if (e === 3) {
    ok++;
}

return ok === 5;
