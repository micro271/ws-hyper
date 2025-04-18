CREATE TYPE ROLE AS ENUM ("Admin", "User");

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    name TEXT,
    surname TEXT,
    email TEXT,
    phone TEXT,
    role ROLE
);

CREATE TABLE IF NOT EXISTS user_tvshow (
    id_users UUID,
    id_tvshow UUID,

    PRIMARY KEY(id_users, id_tvshow),
    FOREIGN KEY(id_users) REFERENCES users (id) ON DELETE SET NULL ON UPDATE CASCADE,
    FOREIGN KEY(id_tvshow) REFERENCES tv_show (id) ON DELETE SET NULL ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS tv_show (
    id UUID,
    name TEXT,
    description TEXT
);

CREATE TABLE IF NOT EXISTS files (
    id UUID,
    create_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    name TEXT,
    stem TEXT,
    extension TEXT,
    elapsed_upload BIGINT,
    id_tvshow UUID,
    PRIMARY KEY(id),
    FOREIGN KEY(id_tvshow) REFERENCES tv_show (id) ON DELETE NO ACTION ON UPDATE NO ACTION
);