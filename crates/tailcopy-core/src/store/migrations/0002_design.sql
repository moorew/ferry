-- Fields the redesigned UI reads (design handoff section 9, D-021). All local:
-- nothing here changes the wire format.

-- How many times the clip was pasted or copied from TailCopy, on any device. Derived
-- from ClipUsed ops like last_used_hlc, so it converges.
ALTER TABLE clips ADD COLUMN use_count INTEGER NOT NULL DEFAULT 0;
-- When this device first held the clip, by its own clock (trail latency).
ALTER TABLE clips ADD COLUMN received_ms INTEGER;

UPDATE clips SET use_count = (
  SELECT count(*) FROM ops WHERE ops.clip_id = clips.id AND ops.kind = 2
) WHERE local_only = 0 AND deleted_hlc IS NULL;
UPDATE clips SET received_ms = created_hlc >> 16;

-- Colour in join order (0 to 5 cycle in the UI), assigned once and never recomputed.
ALTER TABLE devices ADD COLUMN color_index INTEGER;
-- laptop, desktop, server or board; NULL means the default for its role.
ALTER TABLE devices ADD COLUMN device_type TEXT;
ALTER TABLE devices ADD COLUMN os TEXT;
-- Per-device sync switches (used from M2).
ALTER TABLE devices ADD COLUMN send_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE devices ADD COLUMN receive_enabled INTEGER NOT NULL DEFAULT 1;

UPDATE devices SET color_index = 0 WHERE is_self = 1;
UPDATE devices SET color_index = (
  SELECT count(*) FROM devices d WHERE d.is_self = 0 AND d.rowid <= devices.rowid
) WHERE is_self = 0;
