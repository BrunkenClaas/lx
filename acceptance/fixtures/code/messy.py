def calculate_total(items):
    total = 0
    for item in items:
        total = total + item['price'] * item['qty']
    return total

class User:
    def __init__(self, name, email):
        self.name = name
        self.email = email
        self.created = None
        self.updated = None

    def update_profile(self, name, email):
        self.name = name
        self.email = email
        self.updated = time.time()

    # TODO: add password hashing
    def set_password(self, password):
        self.password = password

def validate_email(email):
    if '@' in email:
        return True
    return False

def get_user_by_id(user_id):
    import sqlite3
    conn = sqlite3.connect('data.db')
    cursor = conn.cursor()
    cursor.execute('SELECT * FROM users WHERE id = ' + str(user_id))
    result = cursor.fetchone()
    conn.close()
    return result

def process_order(order):
    x = calculate_total(order['items'])
    print('Order total:', x)
    # TODO: validate inventory
    # TODO: apply discount
    if order['user']['email']:
        # TODO: send confirmation email
        pass
    return x
