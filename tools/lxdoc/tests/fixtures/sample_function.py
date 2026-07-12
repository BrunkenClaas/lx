def add(a, b):
    return a + b


def multiply(x, y):
    return x * y


class Calculator:
    def __init__(self):
        self.history = []

    def compute(self, op, a, b):
        if op == "add":
            result = add(a, b)
        elif op == "mul":
            result = multiply(a, b)
        else:
            raise ValueError(f"Unknown operation: {op}")
        self.history.append((op, a, b, result))
        return result
