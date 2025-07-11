CREATE DATABASE programas;

CREATE TYPE ROL AS ENUM ('Producer', 'Administrator', 'Operator');
CREATE TYPE ESTADO AS ENUM ('Active', 'Inactive');
CREATE TYPE VERBS AS ENUM ('PutFile', 'DeleteFile', 'Read', 'CreateUser', 'ModifyUser', 'CreateProgram', 'ModifyProgram', 'All');

CREATE TABLE IF NOT EXISTS users (
    id UUID DEFAULT gen_random_uuid(),
    username TEXT UNIQUE,
    passwd TEXT,
    email TEXT,
    verbos VERBS[],
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
    user_id TEXT,
    name TEXT UNIQUE,
    description TEXT,
    PRIMARY KEY (user_id, name),
    FOREIGN KEY (user_id) REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE
);