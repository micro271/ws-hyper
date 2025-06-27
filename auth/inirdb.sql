
CREATE DATABASE programas;

CREATE TYPE IF NOT EXISTS ROL AS ENUM ('Productor', 'Administrador', 'Operador');
CREATE TYPE IF NOT EXISTS ESTADO AS ENUM ('Activo', 'Inactivo');
CREATE TYPE IF NOT EXISTS VERBS AS ENUM ('PutFile', 'DeleteFile', 'Read', 'CreateUser', 'ModifyUser', 'CreateCh', 'ModifyCh', 'CreateProgram', 'ModifyProgram', 'All');

CREATE TABLE IF NOT EXISTS usuarios (
    id UUID,
    usuario TEXT unique,
    passwd TEXT,
    email TEXT,
    verbos VERBS,
    estado ESTADO,
    rol ROL,
    resource TEXT,
    PRIMARY KEY (id),
);

CREATE TABLE IF NOT EXISTS programa (
    icon TEXT,
    id_usuario UUID,
    nombre TEXT,
    descripcion TEXT,
    canal UUID,
    PRIMARY KEY (id_usuario, nombre),
);

CREATE TABLE IF NOT EXISTS canal (
    id UUID,
    nombre TEXT,
    descripcion TEXT,
    icon TEXT
);

CREATE TABLE IF NOT EXISTS programa_canal (
    id_usuario UUID,
    nombre TEXT,
    id_canal UUID,
    PRIMARY KEY (id_usuario, id_canal, nombre),
    FOREIGN KEY (id_usuario, nombre) REFERENCES programa (id_usuario, nombre),
    FOREIGN KEY (id_canal) REFERENCES canal (id)
);