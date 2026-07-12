CREATE TABLE customers (
    id          INT PRIMARY KEY,
    name        VARCHAR(255) NOT NULL,
    email       VARCHAR(255) UNIQUE NOT NULL,
    created_at  TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE orders (
    id          INT PRIMARY KEY,
    customer_id INT NOT NULL REFERENCES customers(id),
    total_usd   DECIMAL(12, 2) NOT NULL,
    status      VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at  TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE order_items (
    id         INT PRIMARY KEY,
    order_id   INT NOT NULL REFERENCES orders(id),
    product_id INT NOT NULL,
    quantity   INT NOT NULL,
    unit_price DECIMAL(10, 2) NOT NULL
);
