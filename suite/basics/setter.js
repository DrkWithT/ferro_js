// test object setter

var data = {
    x: undefined,
    set X(arg) {
        this.x = arg;
        return this.x;
    }
};

data.X = 1;

data.x;
