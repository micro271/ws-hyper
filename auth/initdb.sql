CREATE DATABASE buckets;

CREATE TYPE PERMISSIONS AS ENUM ('Put', 'Get', 'Delete');
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
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS users_buckets {
    bucket TEXT,
    user_id UUID,
    permissions PERMISSIONS[],
    PRIMARY KEY (bucket, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL ON UPDATE CASCADE,
    FOREIGN KEY (program_id) REFERENCES program(id) ON DELETE SET NULL ON UPDATE CASCADE
}


CREATE OR REPLACE FUNCTION notify_row_change()
RETURNS trigger AS $$
DECLARE
    payload json;
BEGIN
    IF TG_OP = 'INSERT' THEN
        payload := json_build_object(
            'operation', 'New',
            'bucket', NEW.name
        );
    ELSIF TG_OP = 'UPDATE' AND OLD.name IS DISTINCT FROM NEW.name THEN
        payload := json_build_object(
            'operation', 'Rename',
            'old_bucket', OLD.name,
            'bucket', NEW.name
        );
    ELSIF TG_OP = 'DELETE' THEN
        payload := json_build_object(
            'operation', 'Delete',
            'bucket', OLD.name,
        );
    END IF;

    PERFORM pg_notify('bucket_changed', payload::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_tabla_notif
AFTER INSERT OR UPDATE OR DELETE ON buckets
FOR EACH ROW
EXECUTE FUNCTION notify_row_change();