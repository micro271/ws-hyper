CREATE DATABASE programas;

CREATE TYPE ROL AS ENUM ('Producer', 'Administrator', 'Operator', 'SuperUs');
CREATE TYPE ESTADO AS ENUM ('Active', 'Inactive');

CREATE TABLE IF NOT EXISTS users (
    id UUID DEFAULT gen_random_uuid(),
    username TEXT UNIQUE,
    passwd TEXT,
    email TEXT,
    user_state ESTADO,
    phone TEXT,
    role ROL,
    resources TEXT,
    description TEXT,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS programs (
    id UUID,
    icon TEXT,
    user_id UUID,
    name TEXT UNIQUE,
    description TEXT,
    PRIMARY KEY (user_id, name),
    FOREIGN KEY (user_id) REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE
);