CREATE DATABASE programas;

CREATE TYPE IF NOT EXISTS ROL AS ENUM ('Producer', 'Administrator', 'Operator');
CREATE TYPE IF NOT EXISTS ESTADO AS ENUM ('Active', 'Inactive');
CREATE TYPE IF NOT EXISTS VERBS AS ENUM ('PutFile', 'DeleteFile', 'Read', 'CreateUser', 'ModifyUser', 'CreateCh', 'ModifyCh', 'CreateProgram', 'ModifyProgram', 'All');

CREATE TABLE IF NOT EXISTS user (
    username TEXT unique,
    passwd TEXT,
    email TEXT,
    verbos VERBS,
    user_state ESTADO,
    phone TEXT,
    role ROL,
    resource TEXT,
    PRIMARY KEY (id),
);

CREATE TABLE IF NOT EXISTS program (
    icon TEXT,
    username TEXT,
    name TEXT,
    description TEXT,
    PRIMARY KEY (username, name),
    FOREIGN KEY (username) REFERENCES user(username)
);

CREATE TABLE IF NOT EXISTS channel (
    id UUID,
    name TEXT,
    descripcion TEXT,
    icon TEXT
);

CREATE TABLE IF NOT EXISTS programa_canal (
    id_user UUID,
    name TEXT,
    PRIMARY KEY (name)
    FOREIGN KEY ()
);