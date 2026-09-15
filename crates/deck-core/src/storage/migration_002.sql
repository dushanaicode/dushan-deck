CREATE TABLE float_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    preferences TEXT NOT NULL,
    width INTEGER NOT NULL CHECK (width >= 220 AND width <= 7680),
    height INTEGER NOT NULL CHECK (height >= 260 AND height <= 4320),
    background TEXT
) STRICT;
PRAGMA user_version = 2;
