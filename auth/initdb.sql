CREATE DATABASE programas;

CREATE TYPE ROL AS ENUM ('Producer', 'Administrator', 'Operator');
CREATE TYPE ESTADO AS ENUM ('Active', 'Inactive');
CREATE TYPE VERBS AS ENUM ('PutFile', 'DeleteFile', 'Read', 'CreateUser', 'ModifyUser', 'CreateCh', 'ModifyCh', 'CreateProgram', 'ModifyProgram', 'All');

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
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS program (
    id UUID,
    icon TEXT,
    username TEXT,
    name TEXT UNIQUE,
    description TEXT,
    PRIMARY KEY (username, name),
    FOREIGN KEY (username) REFERENCES users(username) ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS channel (
    id UUID,
    name TEXT UNIQUE,
    descripcion TEXT,
    icon TEXT,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS programa_canal (
    username TEXT,
    name TEXT,
    id_channel UUID,
    PRIMARY KEY (username, name, id_channel),
    FOREIGN KEY (username, name) REFERENCES program (username, name) ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (id_channel) REFERENCES channel (id) ON UPDATE CASCADE ON DELETE CASCADE
);