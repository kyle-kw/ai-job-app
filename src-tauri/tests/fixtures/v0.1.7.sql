CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);
INSERT INTO schema_migrations(version, applied_at) VALUES
  (1, '2026-07-01T00:00:00+08:00'),
  (2, '2026-07-01T00:00:00+08:00'),
  (3, '2026-07-01T00:00:00+08:00'),
  (4, '2026-07-01T00:00:00+08:00'),
  (5, '2026-07-01T00:00:00+08:00');

CREATE TABLE ai_providers (
  id TEXT PRIMARY KEY,
  payload_json TEXT NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  payload_json TEXT NOT NULL
);
INSERT INTO app_settings(key, payload_json) VALUES (
  'settings',
  '{"advancedMode":false,"telemetry":true,"privacyAcknowledged":true}'
);
