INSERT INTO users (id, email, password_hash, display_name) VALUES
    ('11111111-1111-1111-1111-111111111111', 'alice@mail.com', '$2b$...alice', 'Alice'),
    ('22222222-2222-2222-2222-222222222222', 'bob@mail.com',   '$2b$...bob',   'Bob'),
    ('33333333-3333-3333-3333-333333333333', 'carol@mail.com', '$2b$...carol', 'Carol'),
    ('44444444-4444-4444-4444-444444444444', 'dave@mail.com',  '$2b$...dave',  'Dave'),
    ('55555555-5555-5555-5555-555555555555', 'eve@mail.com',   '$2b$...eve',   'Eve');

INSERT INTO listings (id, seller_id, title, price_cents, status) VALUES
    ('a1a1a1a1-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111', 'iPhone 12',       30000, 'active'),
    ('a1a1a1a1-0000-0000-0000-000000000002', '22222222-2222-2222-2222-222222222222', 'iPad Air',        25000, 'sold'),
    ('a1a1a1a1-0000-0000-0000-000000000003', '33333333-3333-3333-3333-333333333333', 'Nintendo Switch', 18000, 'active'),
    ('a1a1a1a1-0000-0000-0000-000000000004', '44444444-4444-4444-4444-444444444444', 'Drone',           40000, 'active'),
    ('a1a1a1a1-0000-0000-0000-000000000005', '55555555-5555-5555-5555-555555555555', 'Camera',          22000, 'active');

INSERT INTO orders (id, buyer_id, listing_id, status, total_cents) VALUES
    ('b2b2b2b2-0000-0000-0000-000000000001',
     '33333333-3333-3333-3333-333333333333',
     'a1a1a1a1-0000-0000-0000-000000000002',
     'paid', 25000);

INSERT INTO swaps (id, initiator_id, initiator_listing_id, recipient_id, recipient_listing_id, status) VALUES
    ('c3c3c3c3-0000-0000-0000-000000000001',
     '44444444-4444-4444-4444-444444444444',
     'a1a1a1a1-0000-0000-0000-000000000004',
     '55555555-5555-5555-5555-555555555555',
     'a1a1a1a1-0000-0000-0000-000000000005',
     'accepted');

INSERT INTO transactions (id, order_id, swap_id, amount_cents, payment_status) VALUES
    ('d4d4d4d4-0000-0000-0000-000000000001',
     'b2b2b2b2-0000-0000-0000-000000000001', NULL, 25000, 'succeeded'),
    ('d4d4d4d4-0000-0000-0000-000000000002',
     NULL, 'c3c3c3c3-0000-0000-0000-000000000001', 0, 'succeeded');
