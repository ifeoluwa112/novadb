CREATE INDEX IF NOT EXISTS idx_listings_active_feed
    ON listings (created_at DESC, id DESC)
    WHERE status = 'active';

DROP INDEX IF EXISTS idx_listings_status;

INSERT INTO listings (seller_id, title, price_cents)
SELECT (SELECT id FROM users WHERE email = 'zed@mail.com'), 'Test gadget ' || n, 1000 + n
FROM generate_series(1, 100) AS n;