var foo = {
    x: 1,
    get x() {
        return this.x;
    },
    set x(arg) {
        this.x = arg;
        return this.x;
    }
};

foo.x = 2;

if (foo.x !== 2) {
    return false;
}

var extra = foo.x * foo.x + foo.x * foo.x;

if (extra !== 8) {
    return false;
}

return true;
