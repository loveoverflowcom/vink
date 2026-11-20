-- A reclaimed delivery may run with a different From or template. Never submit a different
-- payload under the same provider key. Store only a digest, and only while the code is live.
ALTER TABLE mail_deliveries
    ADD COLUMN payload_fingerprint bytea,
    ADD CONSTRAINT mail_deliveries_payload_while_pending CHECK (
        payload_fingerprint IS NULL OR (state = 'pending' AND octet_length(payload_fingerprint) = 32)
    );
