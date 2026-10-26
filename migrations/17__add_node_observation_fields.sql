-- Separates node identity from observed state.
--
-- Until now a node was looked up by private_ip, which is an *observation*: it is
-- cleared by Node::reset(), and it changes whenever a slot is re-provisioned. The
-- key by which a row was found could therefore disappear while the row itself
-- remained valid, which is why reset() writes an empty string rather than NULL.
-- instance_id is the provider's stable handle for the machine occupying a slot,
-- and is the identity callers should prefer.
--
-- observed_at records when instance_id, private_ip and public_ip were last
-- confirmed against the provider. It makes staleness visible to a reader rather
-- than assumed. NULL means never observed.
ALTER TABLE nodes ADD COLUMN instance_id TEXT NULL;
ALTER TABLE nodes ADD COLUMN observed_at DATETIME NULL;
