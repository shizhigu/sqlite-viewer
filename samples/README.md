# Sample databases

## `ecommerce.sqlite` — 3.38 MB, ~52k rows

A realistic e-commerce dataset designed to exercise the viewer and CLI across a broad feature surface.

### Schema

| Table | Rows | Purpose |
|---|---|---|
| `customers` | 500 | People who buy things. Unique email, CHECK on loyalty tier. |
| `addresses` | ~700 | 1:N from customers. ON DELETE CASCADE. |
| `categories` | 16 | Self-referential hierarchy (top-level + sub-categories). |
| `products` | 200 | Includes a `thumbnail BLOB` column and CHECK constraints. |
| `tags` | 25 | Descriptive tags. |
| `product_tags` | ~700 | M:N join with a **composite primary key**. |
| `orders` | 10,000 | Includes CHECK on status enum. |
| `order_items` | ~35,000 | Large table — good for pagination. RESTRICT FK. |
| `reviews` | 5,000 | 1-5 rating with CHECK constraint. |

Plus:

- **Views:** `v_top_customers`, `v_category_revenue`, `v_recent_orders`.
- **Triggers:** `trg_products_updated_at` (after update).
- **Indexes:** 6 including composite (`orders(customer_id, placed_at)`, `reviews(product_id, posted_at)`).

### Regenerate

```sh
python3 scripts/make_sample.py
```

Deterministic — the random seed is fixed, so regenerating produces the same data.

### Try it

```sh
# CLI
./target/release/sqlv open     --db samples/ecommerce.sqlite
./target/release/sqlv tables   --db samples/ecommerce.sqlite
./target/release/sqlv schema   --db samples/ecommerce.sqlite orders
./target/release/sqlv query    --db samples/ecommerce.sqlite \
    "SELECT * FROM v_top_customers"

# Desktop (from apps/desktop)
bunx tauri dev
# then File → Open → samples/ecommerce.sqlite
```
