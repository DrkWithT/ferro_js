var a = [42], ok = 0;

if (a[0] === 42) {
    ok++;
}

if (a[1] === undefined) {
    ok++;
}

if (a.length === 1) {
    ok++;
}

a.length++;
a[1] = 7;

if (a[1] === 7 && a.length === 2) {
    ok++;
}

a.length--;

if (a.length === 1) {
    ok++;
}

return ok === 5;
