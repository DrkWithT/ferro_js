// test typeof

var ok = 0;

if (typeof undefined === "undefined") {
    ok++;
}

if (typeof null === "object") {
    ok++;
}

if (typeof true === "boolean") {
    ok++;
}

if (typeof 1 === "number") {
    ok++;
}

if (typeof "ABC" === "string") {
    ok++;
}

if (typeof {} === "object") {
    ok++;
}

return ok === 6;
