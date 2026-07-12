def calculate_discount(price, discount_rate):
    return price * (1 - discount_rate)

def format_price(amount, currency):
    return currency + str(round(amount, 2))

def apply_tax(price, rate):
    return price * (1 + rate)

def process_order(items, tax_rate, discount):
    subtotal = sum(item["price"] * item["qty"] for item in items)
    discounted = calculate_discount(subtotal, discount)
    total = apply_tax(discounted, tax_rate)
    return total
