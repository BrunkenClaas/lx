def calculate_total(items):
    """Calculate the total price for a list of items.

    Args:
        items: list of dicts with 'price' and 'qty' keys.

    Returns:
        float: total price.
    """
    return sum(item["price"] * item["qty"] for item in items)


def validate_email(email):
    """Return True if email contains '@', False otherwise."""
    return "@" in email
