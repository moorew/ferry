-- Device names travel in the Devices frame, last writer wins by this HLC (SPEC 7.3).
ALTER TABLE devices ADD COLUMN name_hlc INTEGER NOT NULL DEFAULT 0;
