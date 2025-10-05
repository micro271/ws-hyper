CREATE DATABASE programas;

CREATE TYPE ROL AS ENUM ('Producer', 'Administrator', 'Operator', 'SuperUs');
CREATE TYPE ESTADO AS ENUM ('Active', 'Inactive');

CREATE TABLE IF NOT EXISTS programas (
    id UUID,
    icon TEXT,
    name TEXT UNIQUE,
    description TEXT,
    PRIMARY KEY (id)
);

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
    programa UUID,
    PRIMARY KEY (id),
    FOREIGN KEY (programa) REFERENCES programas(id) ON DELETE SET NULL ON UPDATE CASCADE
);