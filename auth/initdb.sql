CREATE DATABASE buckets;

CREATE TYPE PERMISSIONS AS ENUM ("Put", "Get", "Delete");
CREATE TYPE ESTADO AS ENUM ('Active', 'Inactive');

CREATE TABLE IF NOT EXISTS buckets (
    name TEXT UNIQUE,
    description TEXT,
    PRIMARY KEY (name)
);

CREATE TABLE IF NOT EXISTS users (
    id UUID DEFAULT gen_random_uuid(),
    username TEXT UNIQUE,
    passwd TEXT,
    email TEXT,
    user_state ESTADO,
    phone TEXT,
    role ROL,
    description TEXT,
    PRIMARY KEY (id),
);

CREATE TABLE IF NOT EXISTS users_buckets {
    bucket TEXT,
    user_id UUID,
    permissions PERMISSIONS[],
    PRIMARY KEY (bucket, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL ON UPDATE CASCADE,
    FOREIGN KEY (program_id) REFERENCES program(id) ON DELETE SET NULL ON UPDATE CASCADE
}