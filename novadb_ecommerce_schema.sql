CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE listings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    seller_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    description     TEXT,
    price_cents     INTEGER NOT NULL CHECK (price_cents >= 0),
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'sold', 'removed')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_listings_seller_id ON listings(seller_id);
CREATE INDEX idx_listings_status    ON listings(status);

CREATE TABLE orders (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    buyer_id        UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    listing_id      UUID NOT NULL UNIQUE REFERENCES listings(id) ON DELETE RESTRICT,
    status          TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'paid', 'cancelled')),
    total_cents     INTEGER NOT NULL CHECK (total_cents >= 0),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_orders_buyer_id ON orders(buyer_id);

CREATE TABLE swaps (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    initiator_id            UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    initiator_listing_id    UUID NOT NULL REFERENCES listings(id) ON DELETE RESTRICT,
    recipient_id            UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    recipient_listing_id    UUID NOT NULL REFERENCES listings(id) ON DELETE RESTRICT,
    status                  TEXT NOT NULL DEFAULT 'proposed'
                                CHECK (status IN ('proposed', 'accepted', 'completed', 'declined')),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CHECK (initiator_id <> recipient_id)
);

CREATE INDEX idx_swaps_initiator_id ON swaps(initiator_id);
CREATE INDEX idx_swaps_recipient_id ON swaps(recipient_id);

CREATE TABLE transactions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id        UUID REFERENCES orders(id) ON DELETE RESTRICT,
    swap_id         UUID REFERENCES swaps(id) ON DELETE RESTRICT,
    amount_cents    INTEGER NOT NULL CHECK (amount_cents >= 0),
    payment_status  TEXT NOT NULL DEFAULT 'initiated'
                        CHECK (payment_status IN ('initiated', 'succeeded', 'failed', 'refunded')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CHECK (
        (order_id IS NOT NULL AND swap_id IS NULL) OR
        (order_id IS NULL AND swap_id IS NOT NULL)
    )
);

CREATE INDEX idx_transactions_order_id ON transactions(order_id);
CREATE INDEX idx_transactions_swap_id  ON transactions(swap_id);
