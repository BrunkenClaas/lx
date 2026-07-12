function add(a, b) {
    return a + b;
}

function multiply(x, y) {
    return x * y;
}

class Calculator {
    constructor() {
        this.history = [];
    }

    compute(op, a, b) {
        let result;
        if (op === "add") {
            result = add(a, b);
        } else if (op === "mul") {
            result = multiply(a, b);
        } else {
            throw new Error(`Unknown operation: ${op}`);
        }
        this.history.push({ op, a, b, result });
        return result;
    }
}

module.exports = { add, multiply, Calculator };
