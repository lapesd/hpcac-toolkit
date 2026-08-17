-- Lets a recovery slot declare its own init commands, so a replacement node can be
-- prepared differently from the node it replaces. Needed when the replacement is a
-- different instance family: a g7e standing in for an r8i has a local NVMe instance
-- store to format and mount, which the original never had.
--
-- NULL preserves existing behaviour. An in-place replacement keeps the shell_commands
-- already attached to its node row (the row is updated, not recreated, so they carry
-- over); scale-up nodes copy the commands of the slot they fan out from.
ALTER TABLE recovery_nodes ADD COLUMN init_commands TEXT NULL;
