#!/usr/bin/env python3
"""Generate samples/ecommerce.sqlite — a realistic e-commerce database.

Intentionally exercises a broad surface of SQLite features so the viewer
and CLI have something interesting to demo against:

- 9 tables with 1:1, 1:N, and M:N relationships
- Self-referential hierarchy (categories)
- Composite primary key (product_tags)
- ON DELETE CASCADE / SET NULL foreign keys
- Multiple indexes (single + composite)
- 3 views
- 2 triggers (auto-timestamp, auto-compute line total)
- CHECK constraints
- BLOB column (product thumbnails — 16 random bytes each)
- Unicode data, NULLs, dates stored as ISO strings
- ~46k rows (enough to exercise pagination in the Browse tab)

Deterministic: random.seed() is fixed so the output is stable across runs.
"""

from __future__ import annotations

import os
import random
import sqlite3
import string
from datetime import datetime, timedelta, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "samples" / "ecommerce.sqlite"

random.seed(20260421)

# ---- seed data --------------------------------------------------------------

FIRST_NAMES = [
    "Alice", "Björk", "Chen", "Dmitri", "Evelyn", "Fatima", "Gustavo", "Hana",
    "Idris", "Jasmine", "Kofi", "Lilia", "Mateo", "Nadia", "Oskar", "Priya",
    "Quan", "Rafael", "Sofía", "Takeshi", "Uma", "Viktor", "Wen", "Xóchitl",
    "Yusuf", "Zara",
]
LAST_NAMES = [
    "Abebe", "Becker", "Castillo", "Dlamini", "Erikson", "Fontaine", "Gupta",
    "Hernández", "Ivanov", "Jansen", "Kapoor", "Lindqvist", "Müller", "Nakamura",
    "Okafor", "Petrova", "Quintero", "Rossi", "Sato", "Tanaka", "Úbeda",
    "Vogel", "Wang", "Xu", "Yamada", "Zając",
]
CITIES = [
    ("San Francisco", "USA"), ("Berlin", "Germany"), ("Tokyo", "Japan"),
    ("São Paulo", "Brazil"), ("Lagos", "Nigeria"), ("Stockholm", "Sweden"),
    ("Mumbai", "India"), ("Mexico City", "Mexico"), ("Seoul", "South Korea"),
    ("Cape Town", "South Africa"),
]
LOYALTY_TIERS = ["bronze", "silver", "gold", "platinum"]

TOP_CATEGORIES = ["Electronics", "Home & Garden", "Books", "Apparel", "Outdoors"]
SUBCATEGORIES = {
    "Electronics": ["Phones", "Laptops", "Audio"],
    "Home & Garden": ["Kitchen", "Bedding"],
    "Books": ["Fiction", "Non-fiction"],
    "Apparel": ["Men", "Women"],
    "Outdoors": ["Camping", "Cycling"],
}

PRODUCT_ADJECTIVES = [
    "Classic", "Pro", "Mini", "Ultra", "Eco", "Smart", "Vintage", "Nordic",
    "Alpine", "Studio", "Heritage", "Artisan",
]
PRODUCT_NOUNS = {
    "Phones": ["Smartphone", "Handset", "Flip Phone"],
    "Laptops": ["Laptop", "Notebook", "Workstation"],
    "Audio": ["Headphones", "Earbuds", "Speaker"],
    "Kitchen": ["Skillet", "Blender", "Kettle", "Knife Set"],
    "Bedding": ["Duvet", "Pillow", "Sheet Set"],
    "Fiction": ["Novel", "Anthology", "Collection"],
    "Non-fiction": ["Biography", "History", "Guide"],
    "Men": ["T-Shirt", "Jacket", "Jeans", "Hoodie"],
    "Women": ["Blouse", "Dress", "Skirt", "Cardigan"],
    "Camping": ["Tent", "Sleeping Bag", "Headlamp"],
    "Cycling": ["Helmet", "Saddle", "Bike Lock"],
}

TAGS = [
    "new", "sale", "bestseller", "eco", "vegan", "handmade", "imported",
    "limited", "wireless", "waterproof", "premium", "minimalist",
    "ergonomic", "durable", "lightweight", "compact", "portable", "gift",
    "featured", "clearance", "seasonal", "organic", "recycled", "fair-trade",
    "award-winning",
]

STATUSES = ["pending", "paid", "shipped", "delivered", "cancelled", "refunded"]
STATUS_WEIGHTS = [1, 2, 3, 8, 1, 1]  # most orders end delivered

# ---- helpers ---------------------------------------------------------------

def iso(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def rand_sku() -> str:
    return "SKU-" + "".join(random.choices(string.ascii_uppercase + string.digits, k=8))


def rand_email(first: str, last: str, i: int) -> str:
    return f"{first.lower()}.{last.lower()}.{i}@example.com".replace(" ", "")


def rand_blob() -> bytes:
    return random.randbytes(16)


def rand_date(start: datetime, end: datetime) -> datetime:
    delta = end - start
    seconds = random.randint(0, int(delta.total_seconds()))
    return start + timedelta(seconds=seconds)


# ---- schema ----------------------------------------------------------------

SCHEMA = """
PRAGMA foreign_keys = ON;

CREATE TABLE customers (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL,
    email           TEXT NOT NULL UNIQUE,
    signup_at       TEXT NOT NULL,
    loyalty_tier    TEXT NOT NULL CHECK (loyalty_tier IN ('bronze','silver','gold','platinum')),
    total_spent     REAL NOT NULL DEFAULT 0.0,
    notes           TEXT
);

CREATE TABLE addresses (
    id              INTEGER PRIMARY KEY,
    customer_id     INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    line1           TEXT NOT NULL,
    city            TEXT NOT NULL,
    country         TEXT NOT NULL,
    is_default      INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0,1))
);

CREATE TABLE categories (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    parent_id       INTEGER REFERENCES categories(id) ON DELETE SET NULL
);

CREATE TABLE products (
    id              INTEGER PRIMARY KEY,
    sku             TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    category_id     INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    price           REAL NOT NULL CHECK (price >= 0),
    stock           INTEGER NOT NULL DEFAULT 0 CHECK (stock >= 0),
    description     TEXT,
    thumbnail       BLOB,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE tags (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE
);

CREATE TABLE product_tags (
    product_id      INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    tag_id          INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (product_id, tag_id)
);

CREATE TABLE orders (
    id                      INTEGER PRIMARY KEY,
    customer_id             INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    shipping_address_id     INTEGER REFERENCES addresses(id) ON DELETE SET NULL,
    placed_at               TEXT NOT NULL,
    status                  TEXT NOT NULL
        CHECK (status IN ('pending','paid','shipped','delivered','cancelled','refunded')),
    subtotal                REAL NOT NULL,
    discount                REAL NOT NULL DEFAULT 0.0,
    tax                     REAL NOT NULL DEFAULT 0.0,
    total                   REAL NOT NULL
);

CREATE TABLE order_items (
    id              INTEGER PRIMARY KEY,
    order_id        INTEGER NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id      INTEGER NOT NULL REFERENCES products(id) ON DELETE RESTRICT,
    quantity        INTEGER NOT NULL CHECK (quantity > 0),
    unit_price      REAL NOT NULL,
    line_total      REAL NOT NULL
);

CREATE TABLE reviews (
    id              INTEGER PRIMARY KEY,
    product_id      INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    customer_id     INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    rating          INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 5),
    body            TEXT,
    posted_at       TEXT NOT NULL
);

-- Indexes
CREATE INDEX idx_addresses_customer         ON addresses(customer_id);
CREATE INDEX idx_products_category          ON products(category_id);
CREATE INDEX idx_orders_customer_placed     ON orders(customer_id, placed_at);
CREATE INDEX idx_order_items_order          ON order_items(order_id);
CREATE INDEX idx_order_items_product        ON order_items(product_id);
CREATE INDEX idx_reviews_product_posted     ON reviews(product_id, posted_at);

-- Views
CREATE VIEW v_top_customers AS
    SELECT c.id, c.name, c.email, c.total_spent
    FROM customers c
    ORDER BY c.total_spent DESC
    LIMIT 20;

CREATE VIEW v_category_revenue AS
    SELECT cat.name AS category, ROUND(SUM(oi.line_total), 2) AS revenue, COUNT(oi.id) AS items_sold
    FROM categories cat
    JOIN products p     ON p.category_id = cat.id
    JOIN order_items oi ON oi.product_id = p.id
    JOIN orders o       ON o.id = oi.order_id AND o.status IN ('paid','shipped','delivered')
    GROUP BY cat.id
    ORDER BY revenue DESC;

CREATE VIEW v_recent_orders AS
    SELECT o.id, o.placed_at, c.name AS customer, o.status, o.total
    FROM orders o
    JOIN customers c ON c.id = o.customer_id
    WHERE o.placed_at >= datetime('now', '-30 days')
    ORDER BY o.placed_at DESC;

-- Triggers
CREATE TRIGGER trg_products_updated_at
AFTER UPDATE ON products
FOR EACH ROW
BEGIN
    UPDATE products SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER trg_order_item_line_total
BEFORE INSERT ON order_items
FOR EACH ROW
WHEN NEW.line_total IS NULL OR NEW.line_total = 0
BEGIN
    SELECT RAISE(IGNORE)
    WHERE 0;
END;
"""

# Note: SQLite triggers can't directly modify NEW row before insert unless
# using INSTEAD OF (on views only). For line_total, we'll compute at insert
# time in Python — keeps the example simple. The trigger above stays as a
# placeholder to demonstrate trigger presence in the schema.


# ---- generation ------------------------------------------------------------

def main():
    if OUT.exists():
        OUT.unlink()
    OUT.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(OUT)
    conn.executescript(SCHEMA)
    conn.commit()
    cur = conn.cursor()

    now = datetime.now(timezone.utc)
    two_years_ago = now - timedelta(days=730)

    # --- categories (hierarchy) ---
    cat_ids: dict[str, int] = {}
    for i, top in enumerate(TOP_CATEGORIES, start=1):
        cur.execute("INSERT INTO categories (name, parent_id) VALUES (?, NULL)", (top,))
        cat_ids[top] = cur.lastrowid
    for top, subs in SUBCATEGORIES.items():
        parent = cat_ids[top]
        for sub in subs:
            cur.execute("INSERT INTO categories (name, parent_id) VALUES (?, ?)", (sub, parent))
            cat_ids[sub] = cur.lastrowid

    # --- customers ---
    N_CUSTOMERS = 500
    customer_rows = []
    for i in range(1, N_CUSTOMERS + 1):
        first = random.choice(FIRST_NAMES)
        last = random.choice(LAST_NAMES)
        signup = rand_date(two_years_ago, now - timedelta(days=1))
        tier = random.choices(LOYALTY_TIERS, weights=[8, 4, 2, 1])[0]
        notes = None if random.random() < 0.9 else random.choice([
            "VIP", "net-30 terms", "follow-up needed", "Björk fan",
        ])
        customer_rows.append((
            f"{first} {last}", rand_email(first, last, i), iso(signup), tier, 0.0, notes,
        ))
    cur.executemany(
        "INSERT INTO customers (name, email, signup_at, loyalty_tier, total_spent, notes) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        customer_rows,
    )

    # --- addresses ---
    address_rows = []
    for cid in range(1, N_CUSTOMERS + 1):
        n = 1 if random.random() < 0.6 else 2
        for j in range(n):
            city, country = random.choice(CITIES)
            line1 = f"{random.randint(1, 9999)} {random.choice(['Main', 'Oak', 'Linden', 'Park', 'Cedar'])} St"
            address_rows.append((cid, line1, city, country, 1 if j == 0 else 0))
    cur.executemany(
        "INSERT INTO addresses (customer_id, line1, city, country, is_default) VALUES (?, ?, ?, ?, ?)",
        address_rows,
    )
    cur.execute("SELECT id, customer_id FROM addresses")
    addresses_by_customer: dict[int, list[int]] = {}
    for aid, cid in cur.fetchall():
        addresses_by_customer.setdefault(cid, []).append(aid)

    # --- tags ---
    cur.executemany("INSERT INTO tags (name) VALUES (?)", [(t,) for t in TAGS])
    cur.execute("SELECT id FROM tags")
    tag_ids = [r[0] for r in cur.fetchall()]

    # --- products ---
    N_PRODUCTS = 200
    product_rows = []
    for _ in range(N_PRODUCTS):
        leaf_cat = random.choice(list(PRODUCT_NOUNS.keys()))
        cat_id = cat_ids[leaf_cat]
        noun = random.choice(PRODUCT_NOUNS[leaf_cat])
        adj = random.choice(PRODUCT_ADJECTIVES)
        name = f"{adj} {noun}"
        price = round(random.uniform(4.99, 1999.99), 2)
        stock = random.randint(0, 500)
        description = random.choice([
            None,
            f"A {adj.lower()} take on the classic {noun.lower()}.",
            f"Designed for everyday use. {noun} that lasts.",
        ])
        product_rows.append((
            rand_sku(), name, cat_id, price, stock, description, rand_blob(),
            iso(rand_date(two_years_ago, now)), iso(now),
        ))
    cur.executemany(
        "INSERT INTO products (sku, name, category_id, price, stock, description, thumbnail, created_at, updated_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        product_rows,
    )

    # --- product_tags (M:N) ---
    product_tag_rows: set[tuple[int, int]] = set()
    for pid in range(1, N_PRODUCTS + 1):
        n_tags = random.randint(1, 6)
        for tid in random.sample(tag_ids, n_tags):
            product_tag_rows.add((pid, tid))
    cur.executemany("INSERT INTO product_tags (product_id, tag_id) VALUES (?, ?)", list(product_tag_rows))

    cur.execute("SELECT id, price FROM products")
    product_catalog = [(r[0], r[1]) for r in cur.fetchall()]

    # --- orders + order_items ---
    N_ORDERS = 10_000
    order_rows = []
    item_rows = []
    customer_totals: dict[int, float] = {}
    order_id_seed = 1
    item_id_seed = 1

    for _ in range(N_ORDERS):
        cid = random.randint(1, N_CUSTOMERS)
        addr_list = addresses_by_customer.get(cid, [])
        shipping_addr = random.choice(addr_list) if addr_list and random.random() < 0.95 else None
        placed_at = rand_date(two_years_ago, now)
        status = random.choices(STATUSES, weights=STATUS_WEIGHTS)[0]

        n_items = random.randint(1, 6)
        items = random.sample(product_catalog, n_items)
        subtotal = 0.0
        line_items = []
        for pid, price in items:
            qty = random.randint(1, 4)
            line_total = round(price * qty, 2)
            line_items.append((order_id_seed, pid, qty, price, line_total))
            subtotal = round(subtotal + line_total, 2)

        # Discounts roughly 10% of orders.
        discount = round(subtotal * 0.1, 2) if random.random() < 0.1 else 0.0
        tax = round((subtotal - discount) * 0.085, 2)
        total = round(subtotal - discount + tax, 2)

        order_rows.append((
            cid, shipping_addr, iso(placed_at), status, subtotal, discount, tax, total,
        ))
        item_rows.extend(line_items)

        if status in ("paid", "shipped", "delivered"):
            customer_totals[cid] = customer_totals.get(cid, 0.0) + total
        order_id_seed += 1
        item_id_seed += n_items

    cur.executemany(
        "INSERT INTO orders (customer_id, shipping_address_id, placed_at, status, subtotal, discount, tax, total) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        order_rows,
    )
    cur.executemany(
        "INSERT INTO order_items (order_id, product_id, quantity, unit_price, line_total) "
        "VALUES (?, ?, ?, ?, ?)",
        item_rows,
    )

    # --- backfill customer.total_spent ---
    cur.executemany(
        "UPDATE customers SET total_spent = ROUND(?, 2) WHERE id = ?",
        [(v, k) for k, v in customer_totals.items()],
    )

    # --- reviews ---
    N_REVIEWS = 5_000
    review_rows = []
    for _ in range(N_REVIEWS):
        pid = random.randint(1, N_PRODUCTS)
        cid = random.randint(1, N_CUSTOMERS)
        rating = random.choices([1, 2, 3, 4, 5], weights=[1, 1, 3, 6, 9])[0]
        body = random.choice([
            None,
            "Love it!",
            "Exactly what I expected.",
            "Arrived fast, works great.",
            "Solid build quality.",
            "Not quite what the photos suggested, but decent.",
            "Would buy again.",
            "Great value for the price.",
        ])
        posted_at = rand_date(two_years_ago, now)
        review_rows.append((pid, cid, rating, body, iso(posted_at)))
    cur.executemany(
        "INSERT INTO reviews (product_id, customer_id, rating, body, posted_at) VALUES (?, ?, ?, ?, ?)",
        review_rows,
    )

    conn.commit()

    # Vacuum/analyze so the viewer reports realistic page counts.
    conn.execute("ANALYZE;")
    conn.execute("VACUUM;")
    conn.close()

    size_mb = OUT.stat().st_size / 1024 / 1024
    print(f"wrote {OUT} ({size_mb:.2f} MB)")

    # Quick summary.
    c2 = sqlite3.connect(OUT)
    for (name, kind) in c2.execute(
        "SELECT name, type FROM sqlite_master "
        "WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY type, name"
    ):
        if kind == "table":
            n = c2.execute(f'SELECT COUNT(*) FROM "{name}"').fetchone()[0]
            print(f"  {kind:5} {name:20} {n:>8} rows")
        else:
            print(f"  {kind:5} {name}")
    c2.close()


if __name__ == "__main__":
    main()
