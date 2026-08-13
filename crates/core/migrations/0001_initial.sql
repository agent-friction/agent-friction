CREATE TABLE permission_events (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	timestamp TEXT NOT NULL, -- ISO 8601
	agent TEXT NOT NULL,
	session_id TEXT,
	repo TEXT,
	model TEXT,
	tool TEXT NOT NULL,
	pattern TEXT NOT NULL,
	decision TEXT NOT NULL,
	context TEXT
);

CREATE TABLE tool_failures (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	timestamp TEXT NOT NULL,
	agent TEXT NOT NULL,
	session_id TEXT,
	repo TEXT,
	model TEXT,
	tool TEXT NOT NULL,
	input TEXT,
	error TEXT NOT NULL,
	source TEXT NOT NULL,
	context TEXT
);

CREATE INDEX idx_perm_repo_tool ON permission_events(repo, tool);
CREATE INDEX idx_perm_timestamp ON permission_events(timestamp);
CREATE INDEX idx_fail_repo_tool ON tool_failures(repo, tool);
CREATE INDEX idx_fail_timestamp ON tool_failures(timestamp);
